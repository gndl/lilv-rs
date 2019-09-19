use crate::node::Node;
use crate::world::ref_node;
use crate::world::World;
use crate::Void;
use std::rc::Rc;

#[link(name = "lilv-0")]
extern "C" {
    fn lilv_scale_point_get_label(point: *const Void) -> *const Void;
    fn lilv_scale_point_get_value(point: *const Void) -> *const Void;
}

pub struct ScalePoint {
    pub(crate) point: *const Void,
    pub(crate) world: Rc<World>,
}

impl ScalePoint {
    pub fn get_label(&self) -> Node {
        ref_node(&self.world, unsafe {
            lilv_scale_point_get_label(self.point)
        })
    }

    pub fn get_value(&self) -> Node {
        ref_node(&self.world, unsafe {
            lilv_scale_point_get_value(self.point)
        })
    }
}