mod missing_two_way_sample;
mod no_distinct;
mod no_not_in;
mod no_union_distinct;
mod no_wildcard_projection;
mod too_large_in_list;

use crate::rule::Rule;
pub use missing_two_way_sample::MissingTwoWaySample;
pub use no_distinct::NoDistinct;
pub use no_not_in::NoNotIn;
pub use no_union_distinct::NoUnionDistinct;
pub use no_wildcard_projection::NoWildcardProjection;
pub use too_large_in_list::TooLargeInList;

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(NoDistinct),
        Box::new(NoNotIn),
        Box::new(NoUnionDistinct),
        Box::new(NoWildcardProjection),
        Box::new(MissingTwoWaySample),
        Box::new(TooLargeInList),
    ]
}
