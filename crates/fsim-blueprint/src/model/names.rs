use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

macro_rules! name_type {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Arc<str>);

        impl $name {
            pub fn new(name: impl AsRef<str>) -> Self {
                Self(Arc::from(name.as_ref()))
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), &*self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self::new(s)
            }
        }
    };
}

name_type!(EntityName);
name_type!(RecipeName);
name_type!(ItemName);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Quality(Arc<str>);

impl Quality {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    pub fn normal() -> Self {
        Self::new("normal")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Quality {
    fn default() -> Self {
        Self::normal()
    }
}
