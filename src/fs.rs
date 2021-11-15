use std::{
    collections::HashMap,
    path::{Component, Path},
};

use anyhow::anyhow;

use crate::users::{AccessMap, Perms, Username};

const ROOT_ID: NodeId = NodeId(0);

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
struct NodeId(u64);

impl NodeId {
    pub fn next(&mut self) -> Self {
        let this = *self;
        self.0 += 1;
        this
    }
}

#[derive(Default)]
struct File {
    content: String,
}
impl File {
    fn read(&self) -> &str {
        &self.content
    }

    fn write(&mut self, data: &str) {
        self.content.clear();
        self.content += data;
    }
}

struct Dir {
    nodes: HashMap<String, NodeId>,
}

impl Dir {
    fn new(id: NodeId, parent_id: NodeId) -> Self {
        let nodes = HashMap::from([(".".into(), id), ("..".into(), parent_id)]);
        Self { nodes }
    }

    fn lookup(&self, name: &str) -> anyhow::Result<NodeId> {
        self.nodes
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("file {} doesn't exist", name))
    }

    fn contains(&self, name: &str) -> bool {
        self.nodes.contains_key(name)
    }

    fn add(&mut self, name: &str, id: NodeId) {
        self.nodes.insert(name.to_string(), id);
    }

    fn rm(&mut self, name: &str) -> Option<NodeId> {
        self.nodes.remove(name)
    }

    fn len(&self) -> usize {
        self.nodes.len() - 2 // minus .. and .
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn entries(&self) -> impl Iterator<Item = (&str, NodeId)> {
        self.nodes.iter().map(|(name, id)| (name.as_str(), *id))
    }
}

enum NodeKind {
    File(File),
    Dir(Dir),
}

#[derive(Debug, Clone, Copy)]
enum NodeTag {
    File,
    Dir,
}

struct Node {
    id: NodeId,
    kind: NodeKind,
    perms: AccessMap,
}

impl Node {
    pub fn new_with_tag(id: NodeId, parent_id: NodeId, tag: NodeTag) -> Self {
        match tag {
            NodeTag::File => Self::new_file(id),
            NodeTag::Dir => Self::new_dir(id, parent_id),
        }
    }

    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            kind,
            perms: Default::default(),
        }
    }

    pub fn new_file(id: NodeId) -> Self {
        Self {
            id,
            kind: NodeKind::File(File::default()),
            perms: Default::default(),
        }
    }

    pub fn new_dir(id: NodeId, parent_id: NodeId) -> Self {
        Self {
            id,
            kind: NodeKind::Dir(Dir::new(id, parent_id)),
            perms: Default::default(),
        }
    }

    pub fn as_file(&self) -> anyhow::Result<&File> {
        match &self.kind {
            NodeKind::File(f) => Ok(f),
            NodeKind::Dir(_) => anyhow::bail!("is not a regular file"),
        }
    }

    pub fn as_dir(&self) -> anyhow::Result<&Dir> {
        match &self.kind {
            NodeKind::File(_) => anyhow::bail!("is not a dir"),
            NodeKind::Dir(d) => Ok(d),
        }
    }

    pub fn as_file_mut(&mut self) -> anyhow::Result<&mut File> {
        match &mut self.kind {
            NodeKind::File(f) => Ok(f),
            NodeKind::Dir(_) => anyhow::bail!("is not a regular file"),
        }
    }

    pub fn as_dir_mut(&mut self) -> anyhow::Result<&mut Dir> {
        match &mut self.kind {
            NodeKind::File(_) => anyhow::bail!("is not a dir"),
            NodeKind::Dir(d) => Ok(d),
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(&self.kind, &NodeKind::File(..))
    }

    pub fn is_dir(&self) -> bool {
        matches!(&self.kind, &NodeKind::Dir(..))
    }
}

pub struct Fs {
    nodes: HashMap<NodeId, Node>,
    node_counter: NodeId,
}

impl Default for Fs {
    fn default() -> Self {
        Self::new()
    }
}

impl Fs {
    pub fn new() -> Self {
        let mut node_counter = ROOT_ID;
        let root_id = node_counter.next();
        let mut nodes = HashMap::new();
        let root = Node::new_dir(root_id, root_id);
        nodes.insert(root_id, root);
        Self {
            nodes,
            node_counter,
        }
    }

    fn resolve_path<'a>(&'a self, path: &Path) -> anyhow::Result<&'a Node> {
        let root = self.nodes.get(&ROOT_ID).expect("root should always exist");

        let node = reduce_segments(path, root, |node, name| self.lookup(node.id, name))?;

        Ok(node)
    }

    fn lookup<'a>(&'a self, id: NodeId, name: &str) -> anyhow::Result<&'a Node> {
        let dir = self.get_node(id).as_dir()?;
        let node_id = dir.lookup(name)?;

        Ok(self.get_node(node_id))
    }

    fn get_node(&self, node_id: NodeId) -> &Node {
        self.nodes.get(&node_id).expect("bug: node should exist")
    }

    fn get_node_mut(&mut self, node_id: NodeId) -> &mut Node {
        self.nodes
            .get_mut(&node_id)
            .expect("bug: node should exist")
    }

    fn create(&mut self, parent_id: NodeId, name: &str, tag: NodeTag) -> anyhow::Result<NodeId> {
        let mut counter = self.node_counter;
        let parent = self.get_node_mut(parent_id).as_dir_mut()?;
        if parent.contains(name) {
            anyhow::bail!("file exists");
        }
        let id = counter.next();
        let node = Node::new_with_tag(id, parent_id, tag);
        parent.add(name, id);
        self.nodes.insert(id, node);

        self.node_counter = counter;
        Ok(id)
    }

    fn resolve_parent_of(&self, path: &Path) -> anyhow::Result<&Node> {
        self.resolve_path(path.parent().unwrap_or_else(|| Path::new(".")))
    }

    // FS

    pub fn read<'a>(&'a self, path: &Path) -> anyhow::Result<&'a str> {
        Ok(self.resolve_path(path)?.as_file()?.read())
    }

    pub fn write(&mut self, path: &Path, data: &str) -> anyhow::Result<()> {
        let id = self.resolve_path(path)?.id;
        let node = self.get_node_mut(id).as_file_mut()?;
        node.write(data);
        Ok(())
    }

    pub fn access(&self, path: &Path) -> anyhow::Result<()> {
        self.resolve_path(path).map(|_| ())
    }

    fn rm_recursive(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.remove(&id) {
            if let NodeKind::Dir(dir) = node.kind {
                dir.entries()
                    .filter(|(name, _)| !matches!(*name, "." | ".."))
                    .for_each(|(_, id)| self.rm_recursive(id))
            }
        }
    }

    pub fn rm(&mut self, path: &Path) -> anyhow::Result<()> {
        let name = filename(path)?;
        let parent = self.resolve_parent_of(path)?;

        let id = parent.as_dir()?.lookup(name)?;
        let node = self.get_node(id);
        if let NodeKind::Dir(dir) = &node.kind {
            if !dir.is_empty() {
                anyhow::bail!("can't remove non-empty directory")
            }
        }

        let parent_id = parent.id;
        let parent = self.get_node_mut(parent_id).as_dir_mut()?;
        let old = parent.rm(name).expect("should exists");
        self.rm_recursive(old);
        Ok(())
    }

    pub fn new_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let parent_id = self.resolve_parent_of(path)?.id;
        let name = filename(path)?;
        self.create(parent_id, name, NodeTag::File)?;

        Ok(())
    }

    pub fn new_dir(&mut self, path: &Path) -> anyhow::Result<()> {
        let parent_id = self.resolve_parent_of(path)?.id;
        let name = filename(path)?;
        self.create(parent_id, name, NodeTag::Dir)?;

        Ok(())
    }

    pub fn exec(&mut self, path: &Path) -> anyhow::Result<()> {
        todo!()
    }
}

fn reduce_segments<F, T>(path: &Path, start: T, mut callback: F) -> anyhow::Result<T>
where
    F: FnMut(T, &str) -> anyhow::Result<T>,
{
    let mut element = start;
    for segment in path.components() {
        let name = match segment {
            Component::Normal(str) => str.to_str().expect("string is utf8"),
            Component::RootDir | Component::Prefix(_) => continue,
            Component::CurDir => ".",
            Component::ParentDir => "..",
        };

        element = callback(element, name)?;
    }
    Ok(element)
}

fn filename(path: &Path) -> anyhow::Result<&str> {
    Ok(path
        .file_name()
        .ok_or_else(|| anyhow!("path can't end in .."))?
        .to_str()
        .expect("valid utf8"))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn lookup_file() {
        let mut fs = Fs::new();
        let id = fs.create(ROOT_ID, "file", NodeTag::File).unwrap();

        let node = fs.get_node(id);
        assert!(node.is_file());

        let node = fs.resolve_path(Path::new("file")).unwrap();
        assert!(node.is_file());
    }

    #[test]
    fn create_write_read() {
        let mut fs = Fs::new();
        fs.new_dir(Path::new("/dir")).unwrap();
        fs.new_file(Path::new("/dir/file")).unwrap();

        let data = "42";
        fs.write(Path::new("/dir/file"), data).unwrap();

        let content = fs.read(Path::new("/dir/file")).unwrap();

        assert_eq!(content, data);
    }

    #[test]
    fn create_write_rm_read() {
        let mut fs = Fs::new();
        fs.new_dir(Path::new("/dir")).unwrap();
        fs.new_file(Path::new("/dir/file")).unwrap();

        let data = "42";
        fs.write(Path::new("/dir/file"), data).unwrap();

        let content = fs.read(Path::new("/dir/file")).unwrap();

        assert_eq!(content, data);

        let res = fs.rm(Path::new("/dir"));
        assert!(res.is_err());
    }
}
