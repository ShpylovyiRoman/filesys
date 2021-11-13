use std::{
    collections::HashMap,
    path::{Component, Path},
};

use anyhow::anyhow;

use crate::users::{Perms, Username};

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
    perms: HashMap<Username, Perms>,
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

    // FS

    pub fn read<'a>(&'a self, path: &Path) -> anyhow::Result<&'a [u8]> {
        todo!()
    }

    pub fn write(&self, path: &Path, data: &[u8]) -> anyhow::Result<()> {
        todo!()
    }

    pub fn access(&self, path: &Path) -> anyhow::Result<()> {
        todo!()
    }

    pub fn rm(&mut self, path: &Path) -> anyhow::Result<()> {
        todo!()
    }

    pub fn new_file(&mut self, path: &Path) -> anyhow::Result<()> {
        todo!()
    }

    pub fn new_dir(&mut self, path: &Path) -> anyhow::Result<()> {
        todo!()
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
}
