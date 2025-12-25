use std::fmt::Debug;
use std::ptr::NonNull;
use std::ffi::CStr;

use lilv_sys as lib;
use lv2_raw::LV2Feature;

use crate::instance::Instance;
use crate::plugin::Plugin;
use crate::node::Node;
use crate::world::World;

unsafe impl Send for State {}
unsafe impl Sync for State {}

pub type UserData = *mut ::std::os::raw::c_void;
pub type Value = *mut ::std::os::raw::c_void;
pub type GetValueClosure = dyn FnMut(&str) -> (u32, u32, Value);

pub trait GetValue {
    fn get_value(&mut self, port_symbol: &str) -> (u32, u32, Value);
}

unsafe extern "C" fn get_value_func(
    port_symbol: *const ::std::os::raw::c_char,
    user_data: *mut ::std::os::raw::c_void,
    size: *mut u32,
    type_: *mut u32,
) -> *const ::std::os::raw::c_void {
    let user_ptr = user_data as *mut Option<Box<dyn GetValue>>;
    let user = unsafe { &mut *user_ptr };
    let port_symbol = unsafe { CStr::from_ptr(port_symbol) };

    if let Some(user) = user {
        let (sz, tp, val) = user.get_value(port_symbol.to_str().unwrap());

        *size = sz;
        *type_ = tp;

        return val;
    }
    *size = 0;
    *type_ = 0;
    std::ptr::null()
}

#[derive(Clone, Debug)]
pub struct State {
    pub(crate) inner: NonNull<lib::LilvState>,
}

impl State {
    pub fn new_from_world(world: &World, map: &mut lv2_raw::LV2UridMap, subject: &Node) -> Option<State> {
        let world = world.as_ptr();
        let map = map as *mut _;
        let subject = subject.inner.as_ptr();

        let state = unsafe { lib::lilv_state_new_from_world(world, map, subject)};

        Some(State {inner: NonNull::new(state)?})
    }

    pub fn new_from_file(world: &World, map: &mut lv2_raw::LV2UridMap, subject: Option<&Node>, path: &str) -> Option<State> {
        let world = world.as_ptr();
        let map = map as *mut _;
        let subject = subject.map_or(std::ptr::null(), |s| s.inner.as_ptr());
        let path = std::ffi::CString::new(path).unwrap();

        let state = unsafe { lib::lilv_state_new_from_file(world, map, subject, path.as_ptr().cast())};

        Some(State {inner: NonNull::new(state)?})
    }

    pub fn new_from_string(world: &World, map: &mut lv2_raw::LV2UridMap, string: &str) -> Option<State> {
        let world = world.as_ptr();
        let map = map as *mut _;
        let string = std::ffi::CString::new(string).unwrap();

        let state = unsafe { lib::lilv_state_new_from_string(world, map, string.as_ptr().cast())};

        Some(State {inner: NonNull::new(state)?})
    }

    pub fn new_from_instance<'a, FS>(
        plugin: &Plugin,
        instance: &Instance,
        map: &mut lv2_raw::LV2UridMap,
        file_dir: Option<&str>,
        copy_dir: Option<&str>,
        link_dir: Option<&str>,
        save_dir: Option<&str>,
        user: Option<&Box<dyn GetValue>>,
        flags: u32,
        features: FS,
    ) -> Option<State>
    where
        FS: IntoIterator<Item = &'a LV2Feature>,
    {
        let plugin = plugin.inner.as_ptr();
        let instance = instance.inner.as_ptr();
        let map = map as *mut _;
        let file_d = std::ffi::CString::new(file_dir.unwrap_or_default()).unwrap();
        let file_dir: *const ::std::os::raw::c_char = file_dir.map_or(std::ptr::null(), |_| file_d.as_ptr().cast());
        let copy_d = std::ffi::CString::new(copy_dir.unwrap_or_default()).unwrap();
        let copy_dir: *const ::std::os::raw::c_char = copy_dir.map_or(std::ptr::null(), |_| copy_d.as_ptr().cast());
        let link_d = std::ffi::CString::new(link_dir.unwrap_or_default()).unwrap();
        let link_dir: *const ::std::os::raw::c_char = link_dir.map_or(std::ptr::null(), |_| link_d.as_ptr().cast());
        let save_d = std::ffi::CString::new(save_dir.unwrap_or_default()).unwrap();
        let save_dir: *const ::std::os::raw::c_char = save_dir.map_or(std::ptr::null(), |_| save_d.as_ptr().cast());
        let get_value: lib::LilvGetPortValueFunc = user.map_or(None, |_| Some(get_value_func));
        let user_data = NonNull::from(&user).as_ptr().cast();

        let features_vec: Vec<*const LV2Feature> = features
            .into_iter()
            .map(|f| f as *const LV2Feature)
            .chain(std::iter::once(std::ptr::null()))
            .collect();

        let state = unsafe {
            lib::lilv_state_new_from_instance(
                plugin,
                instance,
                map,
                file_dir,
                copy_dir,
                link_dir,
                save_dir,
                get_value,
                user_data,
                flags,
                features_vec.as_ptr(),
            )};

        Some(State {inner: NonNull::new(state)?})
    }
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            lib::lilv_state_free(self.inner.as_ptr());
        }
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::convert::TryFrom;
    use std::ffi::{CStr, CString};
    use std::ptr::NonNull;

    use lv2_raw::LV2Feature;

    use crate::world::World;
    use crate::state::State;

    type MapImpl = HashMap<CString, u32>;
    static URID_MAP: &[u8] = b"http://lv2plug.in/ns/ext/urid#map\0";

    extern "C" fn do_map(handle: lv2_raw::LV2UridMapHandle, uri_ptr: *const i8) -> lv2_raw::LV2Urid {
        let handle = handle as *mut MapImpl;
        let map = unsafe { &mut *handle };
        let uri = unsafe { CStr::from_ptr(uri_ptr) };
    
        if let Some(id) = map.get(uri) {
            return *id;
        }
        let id = u32::try_from(map.len()).expect("URID space has exceeded capacity for u32.") + 1;
        map.insert(uri.to_owned(), id);
        id
    }
    
    #[test]
    fn test_new_from_world() {
        let world = World::with_load_all();
        let map = MapImpl::new();
        let map_ptr = NonNull::from(&map);

        let mut lv2_urid_map = lv2_raw::LV2UridMap {
            handle: map_ptr.as_ptr().cast(),
            map: do_map,
        };

        let subject = world.new_uri("http://lv2plug.in/plugins/eg-sampler#sample");

        let state = State::new_from_world(&world, &mut lv2_urid_map, &subject);
        assert!(state.is_none());
    }

    #[test]
    fn test_new_from_file() {
        let world = World::with_load_all();
        let map = MapImpl::new();
        let map_ptr = NonNull::from(&map);

        let mut lv2_urid_map = lv2_raw::LV2UridMap {
            handle: map_ptr.as_ptr().cast(),
            map: do_map,
        };

        let state = State::new_from_file(&world, &mut lv2_urid_map, None, "");
        assert!(state.is_none());
    }

    #[test]
    fn test_new_from_instance() {
        let world = World::with_load_all();
        let map = MapImpl::new();
        let map_ptr = NonNull::from(&map);

        let mut lv2_urid_map = lv2_raw::LV2UridMap {
            handle: map_ptr.as_ptr().cast(),
            map: do_map,
        };
        let map_data_ptr = NonNull::from(&lv2_urid_map);
        let urid_map_feature = LV2Feature {
            uri: URID_MAP.as_ptr().cast(),
            data: map_data_ptr.as_ptr().cast(),
        };

        let features = vec![urid_map_feature];
        let plugin_uri = world.new_uri("http://lv2plug.in/plugins/eg-amp");
        let plugin = world.plugins().plugin(&plugin_uri).unwrap();
        let instance = unsafe{ plugin.instantiate(44100., &features).unwrap()};

        let state = State::new_from_instance(
            &plugin,
            &instance,
            &mut lv2_urid_map,
            None,
            None,
            None,
            None,
            None,
            0,
            &features
        );
        assert!(state.is_some());
    }
}
