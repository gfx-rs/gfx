use std::{iter, mem};
use hal::pass::AttachmentId;
use hal::command::ClearValueRaw;
use smallvec::SmallVec;

#[allow(dead_code)]
pub(crate) fn convert_clear_values_iter<I>(values: I) -> impl Iterator<Item=ClearValueRaw>
where
    I: Iterator<Item=(AttachmentId, ClearValueRaw)>
{
    let dummy = unsafe { mem::zeroed() };
    collect_by_ids(values, dummy)
        .into_iter()
        .chain(iter::repeat(dummy))
}

pub(crate) fn collect_by_ids<I, V>(values: I, dummy: V) -> SmallVec<[V; 16]>
where
    I: Iterator<Item=(AttachmentId, V)>,
    V: Copy
{
    let mut ret = SmallVec::new();

    for (id, value) in values {
        if id == ret.len() {
            ret.push(value);
        } else if id < ret.len() {
            ret[id] = value;
        } else {
            let placeholder_count = id - ret.len();
            ret.extend(iter::repeat(dummy).take(placeholder_count));
            ret.push(value);
        }
    }

    ret
}
