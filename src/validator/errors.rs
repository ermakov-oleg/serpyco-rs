use crate::validator::context::PathChunk;
use crate::validator::InstancePath;

pub(crate) fn into_path(pointer: &InstancePath) -> String {
    let mut path = vec![];
    for chunk in pointer.to_vec() {
        match chunk {
            PathChunk::Property(property) => {
                path.push(property.to_string());
            }
            PathChunk::Index(index) => path.push(index.to_string()),
            PathChunk::PropertyValue(value) => path.push(value.to_string()),
        };
    }
    path.join("/")
}
