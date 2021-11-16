use std::{
    collections::HashMap,
    path::{Component, Path},
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::users::{AccessMap, Op, Perms, UserId};

const ROOT_ID: NodeId = NodeId(0);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct NodeId(u64);

impl NodeId {
    pub fn next(&mut self) -> Self {
        let this = *self;
        self.0 += 1;
        this
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
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
    fn size(&self) -> usize {
        self.content.len()
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
        self.nodes.len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 2 // skip .. and .
    }

    fn entries(&self) -> impl Iterator<Item = (&str, NodeId)> {
        self.nodes.iter().map(|(name, id)| (name.as_str(), *id))
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum NodeKind {
    File(File),
    Dir(Dir),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NodeTag {
    File,
    Dir,
}

#[derive(Debug, Serialize, Deserialize)]
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

    pub fn check_if_allowed(&self, uid: UserId, ops: &[Op]) -> anyhow::Result<()> {
        if self.perms.allows(uid, ops) {
            Ok(())
        } else {
            anyhow::bail!("permission denied")
        }
    }

    pub fn set_perm(&mut self, uid: UserId, perms: impl Into<Perms>) {
        self.perms.set(uid, perms)
    }

    fn tag(&self) -> NodeTag {
        match &self.kind {
            NodeKind::File(_) => NodeTag::File,
            NodeKind::Dir(_) => NodeTag::Dir,
        }
    }

    fn perms_for(&self, uid: UserId) -> Perms {
        self.perms.get(uid)
    }

    fn size(&self) -> usize {
        match &self.kind {
            NodeKind::File(f) => f.size(),
            NodeKind::Dir(d) => d.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeEntry {
    pub tag: NodeTag,
    pub name: String,
    pub perms: Perms,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
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

    /// Transform path into immutable reference
    /// Checks for permissions on every segment of the path
    fn resolve_path<'a>(&'a self, uid: UserId, path: &Path) -> anyhow::Result<&'a Node> {
        let root = self.get_node(uid, ROOT_ID)?;
        let node = reduce_segments(path, root, |node, name| self.lookup(uid, node.id, name))?;
        Ok(node)
    }

    /// Transform path into the mutable reference
    /// Checks for permissions on every segment of the pat
    fn resolve_path_mut<'a>(
        &'a mut self,
        uid: UserId,
        path: &Path,
    ) -> anyhow::Result<&'a mut Node> {
        let id = self.resolve_path(uid, path)?.id;
        self.get_node_mut(uid, id)
    }

    /// Lookup Node in directory with id = parent_id
    fn lookup<'a>(
        &'a self,
        uid: UserId,
        parent_id: NodeId,
        name: &str,
    ) -> anyhow::Result<&'a Node> {
        let dir = self.get_node(uid, parent_id)?.as_dir()?;
        let node_id = dir.lookup(name)?;
        self.get_node(uid, node_id)
    }

    /// Get reference to the node with a given id
    /// Checks permissions
    fn get_node(&self, uid: UserId, node_id: NodeId) -> anyhow::Result<&Node> {
        let node = self.nodes.get(&node_id).expect("bug: node should exist");
        node.check_if_allowed(uid, &[Op::Read])?;
        Ok(node)
    }

    /// Get mutable reference to the node with a given id
    /// Checks permissions
    fn get_node_mut(&mut self, uid: UserId, node_id: NodeId) -> anyhow::Result<&mut Node> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .expect("bug: node should exist");

        node.check_if_allowed(uid, &[Op::Write])?;
        Ok(node)
    }

    /// Creates a new node in a directory with a parent_id
    /// Checks permissions
    fn create(
        &mut self,
        uid: UserId,
        parent_id: NodeId,
        name: &str,
        tag: NodeTag,
    ) -> anyhow::Result<NodeId> {
        let mut counter = self.node_counter;
        let parent = self.get_node_mut(uid, parent_id)?.as_dir_mut()?;
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

    /// Returns immutable reference to the parent of a given path
    fn resolve_parent_of(&self, uid: UserId, path: &Path) -> anyhow::Result<&Node> {
        self.resolve_path(uid, path.parent().unwrap_or_else(|| Path::new(".")))
    }

    /// Returns immutable reference to the parent of a given path
    fn resolve_parent_of_mut(&mut self, uid: UserId, path: &Path) -> anyhow::Result<&mut Node> {
        self.resolve_path_mut(uid, path.parent().unwrap_or_else(|| Path::new(".")))
    }

    // FS

    /// Read a file
    pub fn read<'a>(&'a self, uid: UserId, path: &Path) -> anyhow::Result<&'a str> {
        Ok(self.resolve_path(uid, path)?.as_file()?.read())
    }

    /// Write data to a file. Overrites content
    pub fn write(&mut self, uid: UserId, path: &Path, data: &str) -> anyhow::Result<()> {
        let node = self.resolve_path_mut(uid, path)?.as_file_mut()?;
        node.write(data);
        Ok(())
    }

    /// Remove node and all subnodes
    fn rm_recursive(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.remove(&id) {
            if let NodeKind::Dir(dir) = node.kind {
                dir.entries()
                    .filter(|(name, _)| !matches!(*name, "." | ".."))
                    .for_each(|(_, id)| self.rm_recursive(id))
            }
        }
    }

    /// Remove node. Returns error, if a node is a non-empty directory
    pub fn rm(&mut self, uid: UserId, path: &Path) -> anyhow::Result<()> {
        let name = filename(path)?;
        let parent = self.resolve_parent_of(uid, path)?;

        let id = parent.as_dir()?.lookup(name)?;
        let node = self.get_node(uid, id)?;
        if let NodeKind::Dir(dir) = &node.kind {
            if !dir.is_empty() {
                anyhow::bail!("can't remove non-empty directory")
            }
        }

        let parent_id = parent.id;
        let parent = self.get_node_mut(uid, parent_id)?.as_dir_mut()?;
        let old = parent.rm(name).expect("should exists");
        self.rm_recursive(old);
        Ok(())
    }

    /// Creates new file in a given path. Returns error if the path exists
    pub fn new_file(&mut self, uid: UserId, path: &Path) -> anyhow::Result<()> {
        let parent_id = self.resolve_parent_of_mut(uid, path)?.id;
        let name = filename(path)?;
        self.create(uid, parent_id, name, NodeTag::File)?;
        Ok(())
    }

    /// Creates new directory in a given path. Returns error if the path exists
    pub fn new_dir(&mut self, uid: UserId, path: &Path) -> anyhow::Result<()> {
        let parent_id = self.resolve_parent_of_mut(uid, path)?.id;
        let name = filename(path)?;
        self.create(uid, parent_id, name, NodeTag::Dir)?;
        Ok(())
    }

    /// Executes file in a given path
    pub fn exec(&mut self, uid: UserId, path: &Path) -> anyhow::Result<()> {
        let node = self.resolve_path(uid, path)?;
        node.check_if_allowed(uid, &[Op::Exec])
    }

    /// Sets permissions of a given
    pub fn set_perms(
        &mut self,
        uid: UserId,
        path: &Path,
        perms: impl Into<Perms>,
    ) -> anyhow::Result<()> {
        let node = self.resolve_path_mut(uid, path)?;
        node.check_if_allowed(uid, &[Op::Control])?;
        node.set_perm(uid, perms);
        Ok(())
    }

    /// List entries in a directory
    pub fn ls(&self, uid: UserId, path: &Path) -> anyhow::Result<Vec<NodeEntry>> {
        let dir = self.resolve_path(uid, path)?.as_dir()?;
        let mut entries = Vec::with_capacity(dir.len());

        for (name, id) in dir.entries() {
            let entry = match self.get_node(uid, id) {
                Ok(node) => NodeEntry {
                    tag: node.tag(),
                    name: name.into(),
                    perms: node.perms_for(uid),
                    size: node.size(),
                },
                // TODO: report somehow?
                Err(_) => continue,
            };

            entries.push(entry);
        }

        Ok(entries)
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

    use crate::users::ADMIN_ID;

    use super::*;

    #[test]
    fn create_write_read() {
        let mut fs = Fs::new();
        let uid = ADMIN_ID;
        fs.new_dir(uid, Path::new("/dir")).unwrap();
        fs.new_file(uid, Path::new("/dir/file")).unwrap();

        let data = "42";
        fs.write(uid, Path::new("/dir/file"), data).unwrap();

        let content = fs.read(uid, Path::new("/dir/file")).unwrap();

        assert_eq!(content, data);
    }

    #[test]
    fn create_write_rm_read() {
        let mut fs = Fs::new();
        let uid = ADMIN_ID;
        fs.new_dir(uid, Path::new("/dir")).unwrap();
        fs.new_file(uid, Path::new("/dir/file")).unwrap();

        let data = "42";
        fs.write(uid, Path::new("/dir/file"), data).unwrap();

        let content = fs.read(uid, Path::new("/dir/file")).unwrap();

        assert_eq!(content, data);

        let res = fs.rm(uid, Path::new("/dir"));
        assert!(res.is_err());
    }

    #[test]
    fn create_access_read() {
        let mut fs = Fs::new();
        let uid = ADMIN_ID;

        fs.new_file(uid, Path::new("./file")).unwrap();

        fs.write(uid, Path::new("file"), "my fancy data").unwrap();

        fs.read(UserId::new(12), Path::new("/dir/file"))
            .unwrap_err();
    }
}
