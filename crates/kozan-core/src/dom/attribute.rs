// Attribute collection — dedicated type for element attributes.
//
// Like Chrome's `Vector<Attribute>` with O(n) linear scan.
// Most elements have 0–5 attributes, so linear scan beats HashMap.
// No hardcoded id/class — all attributes are equal.

/// A single attribute (name-value pair).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    name: String,
    value: String,
}

impl Attribute {
    /// Create a new attribute.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// The attribute name.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The attribute value.
    #[inline]
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Set the value, returns the old value.
    pub fn set_value(&mut self, value: impl Into<String>) -> String {
        core::mem::replace(&mut self.value, value.into())
    }
}

/// A collection of attributes.
///
/// Linear scan on a flat `Vec<Attribute>`. O(n) lookup is optimal
/// for the typical 0–5 attributes per element.
#[derive(Clone, Debug, Default)]
pub struct AttributeCollection {
    attrs: Vec<Attribute>,
}

impl AttributeCollection {
    /// Create an empty collection.
    #[must_use]
    pub fn new() -> Self {
        Self { attrs: Vec::new() }
    }

    /// Get an attribute value by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Set an attribute. If it exists, update its value. Otherwise, add it.
    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        if let Some(attr) = self.attrs.iter_mut().find(|a| a.name == name) {
            attr.value = value.into();
        } else {
            self.attrs.push(Attribute::new(name, value));
        }
    }

    /// Remove an attribute by name. Returns the old value if it existed.
    pub fn remove(&mut self, name: &str) -> Option<String> {
        if let Some(pos) = self.attrs.iter().position(|a| a.name == name) {
            Some(self.attrs.swap_remove(pos).value)
        } else {
            None
        }
    }

    /// Check if an attribute exists.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.attrs.iter().any(|a| a.name == name)
    }

    /// Number of attributes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.attrs.len()
    }

    /// Is the collection empty?
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    /// Iterate over all attributes.
    pub fn iter(&self) -> impl Iterator<Item = &Attribute> {
        self.attrs.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_collection() {
        let attrs = AttributeCollection::new();
        assert!(attrs.is_empty());
        assert_eq!(attrs.len(), 0);
        assert!(attrs.get("id").is_none());
    }

    #[test]
    fn set_and_get() {
        let mut attrs = AttributeCollection::new();
        attrs.set("id", "main");
        assert_eq!(attrs.get("id"), Some("main"));
        assert_eq!(attrs.len(), 1);
    }

    #[test]
    fn set_overwrites() {
        let mut attrs = AttributeCollection::new();
        attrs.set("id", "old");
        attrs.set("id", "new");
        assert_eq!(attrs.get("id"), Some("new"));
        assert_eq!(attrs.len(), 1);
    }

    #[test]
    fn remove_returns_old_value() {
        let mut attrs = AttributeCollection::new();
        attrs.set("class", "container");
        let old = attrs.remove("class");
        assert_eq!(old, Some("container".to_string()));
        assert!(attrs.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut attrs = AttributeCollection::new();
        assert_eq!(attrs.remove("missing"), None);
    }

    #[test]
    fn has_check() {
        let mut attrs = AttributeCollection::new();
        attrs.set("data-x", "1");
        assert!(attrs.has("data-x"));
        assert!(!attrs.has("data-y"));
    }

    #[test]
    fn multiple_attributes() {
        let mut attrs = AttributeCollection::new();
        attrs.set("id", "test");
        attrs.set("class", "foo bar");
        attrs.set("data-value", "42");
        assert_eq!(attrs.len(), 3);
        assert_eq!(attrs.get("id"), Some("test"));
        assert_eq!(attrs.get("class"), Some("foo bar"));
        assert_eq!(attrs.get("data-value"), Some("42"));
    }

    #[test]
    fn iterate_attributes() {
        let mut attrs = AttributeCollection::new();
        attrs.set("a", "1");
        attrs.set("b", "2");
        let names: Vec<&str> = attrs.iter().map(|a| a.name()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }
}
