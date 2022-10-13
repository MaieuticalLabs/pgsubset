use crate::config::TransformKind;
use std::collections::HashMap;

use fake::{
    faker::{
        internet::en::{FreeEmail, Username},
        name::en::{FirstName, LastName},
    },
    Fake,
};

pub type Transforms = HashMap<String, TableTransform>;
pub type TableTransform = HashMap<String, Transform>;

pub struct Transform {
    pub func: fn(&str) -> String,
}
impl Transform {
    pub fn new(name: &TransformKind) -> Self {
        match name {
            TransformKind::ClearField => Self { func: clear_field },
            TransformKind::FirstNameEn => Self {
                func: first_name_en,
            },
            TransformKind::LastNameEn => Self { func: last_name_en },
            TransformKind::UsernameEn => Self { func: username_en },
            TransformKind::EmailEn => Self { func: email_en },
            TransformKind::DjangoGarbagePassword => Self {
                func: django_garbage_password,
            },
        }
    }
}

#[derive(Default)]
pub struct IndexedTransforms<'a> {
    pub transforms: HashMap<usize, &'a Transform>,
}
impl<'a> IndexedTransforms<'a> {
    pub fn new(transforms: &'a HashMap<String, Transform>, header: &[String]) -> Self {
        let ts_with_idx: HashMap<_, _> = header
            .iter()
            .enumerate()
            .filter_map(|(idx, h)| transforms.get(h).map(|t| (idx, t)))
            .collect();

        Self {
            transforms: ts_with_idx,
        }
    }
}

pub fn clear_field(_input: &str) -> String {
    "".into()
}

pub fn first_name_en(_input: &str) -> String {
    FirstName().fake()
}

pub fn last_name_en(_input: &str) -> String {
    LastName().fake()
}

pub fn email_en(_input: &str) -> String {
    FreeEmail().fake()
}

pub fn username_en(_input: &str) -> String {
    Username().fake()
}

pub fn django_garbage_password(_input: &str) -> String {
    "!asdfgghjwetrrytrytr453546jyuiEEHGH".into()
}
