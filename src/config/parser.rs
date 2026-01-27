use kdl::{KdlDocument, KdlNode};
use std::{fs, path::Path};

pub struct KdlConfig {
    doc: KdlDocument,
}

impl KdlConfig {
    pub fn from_path(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.is_dir() {
            return Err(format!("{} is a directory, expected a .kdl file", path.display()).into());
        }
        let content = fs::read_to_string(path)?;
        let doc: KdlDocument = content.parse()?;
        Ok(Self { doc })
    }

    /// Returns a reader for a top-level section node (e.g. "commands", "theme").
    pub fn section(&self, name: &str) -> Option<NodeReader<'_>> {
        self.doc.get(name).map(NodeReader)
    }
}

/// Thin wrapper around a KDL node that provides ergonomic, typed property access.
/// Callers never touch raw KDL types.
pub struct NodeReader<'a>(&'a KdlNode);

impl<'a> NodeReader<'a> {
    /// The node's identifier — e.g. "tclean" for a command block.
    pub fn name(&self) -> &str {
        self.0.name().value()
    }

    /// Iterate over the direct children of this node.
    pub fn children(&self) -> impl Iterator<Item = NodeReader<'_>> {
        self.0
            .children()
            .into_iter()
            .flat_map(|doc| doc.nodes())
            .map(NodeReader)
    }

    /// Get a named string property from either:
    ///   - a KDL property:  `key "value"`   → node argument
    ///   - a KDL child:     `key { ... }`   → first arg of child node
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.0
            .children()?
            .get(key)?
            .entries()
            .first()?
            .value()
            .as_string()
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.0
            .children()?
            .get(key)?
            .entries()
            .first()?
            .value()
            .as_bool()
    }
}
