use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    fmt::Debug,
    marker::PhantomData,
};

use crate::lua_engine::lua_stdlib;
use mlua::{FromLua, Lua, ToLua};

use super::*;

macro_rules! impl_type {
    () => {};
    ([$trait:ty, $key_type:ident, $fn:ident] ~ $t:ident) => {
        impl $trait for $t {
            fn $fn() -> $key_type {
                $key_type::$t
            }
            fn name() -> &'static str {
                stringify!($t)
            }
        }
    };
}

pub trait ChannelKey:
    slotmap::Key + Default + Debug + Clone + Copy + Sized + FromToLua + 'static
{
    fn key_type() -> ChannelKeyType;
    fn name() -> &'static str;
}
impl_type!([ChannelKey, ChannelKeyType, key_type] ~ VertexId);
impl_type!([ChannelKey, ChannelKeyType, key_type] ~ FaceId);
impl_type!([ChannelKey, ChannelKeyType, key_type] ~ HalfEdgeId);

pub trait ChannelValue: Default + Debug + Clone + Copy + Sized + FromToLua + 'static {
    fn value_type() -> ChannelValueType;
    fn name() -> &'static str;
}
impl_type!([ChannelValue, ChannelValueType, value_type] ~ Vec2);
impl_type!([ChannelValue, ChannelValueType, value_type] ~ Vec3);
impl_type!([ChannelValue, ChannelValueType, value_type] ~ Vec4);
impl_type!([ChannelValue, ChannelValueType, value_type] ~ f32);
impl_type!([ChannelValue, ChannelValueType, value_type] ~ bool);

pub trait FromToLua {
    fn cast_to_lua(self, lua: &Lua) -> mlua::Value;
    fn cast_from_lua(value: mlua::Value, lua: &Lua) -> Result<Self>
    where
        Self: Sized;
}

macro_rules! impl_from_to_lua {
    (wrapped $t:ident) => {
        impl FromToLua for $t {
            fn cast_to_lua<'lua>(self, lua: &'lua Lua) -> mlua::Value {
                lua_stdlib::$t(self).to_lua(lua).unwrap()
            }

            fn cast_from_lua(value: mlua::Value, lua: &Lua) -> Result<Self> {
                let value: lua_stdlib::$t = FromLua::from_lua(value, lua)?;
                Ok(value.0)
            }
        }
    };
    (flat $t:ident) => {
        impl FromToLua for $t {
            fn cast_to_lua<'lua>(self, lua: &'lua Lua) -> mlua::Value {
                self.to_lua(lua).unwrap()
            }

            fn cast_from_lua(value: mlua::Value, lua: &Lua) -> Result<Self> {
                let value: $t = FromLua::from_lua(value, lua)?;
                Ok(value)
            }
        }
    };
}
impl_from_to_lua!(wrapped Vec2);
impl_from_to_lua!(wrapped Vec3);
impl_from_to_lua!(wrapped Vec4);
impl_from_to_lua!(flat f32);
impl_from_to_lua!(flat bool);
impl_from_to_lua!(flat VertexId);
impl_from_to_lua!(flat FaceId);
impl_from_to_lua!(flat HalfEdgeId);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[rustfmt::skip]
pub enum ChannelKeyType { VertexId, FaceId, HalfEdgeId }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[rustfmt::skip]
#[allow(non_camel_case_types)]
pub enum ChannelValueType { Vec2, Vec3, Vec4, f32, bool }

#[derive(Clone, Debug)]
pub struct Channel<K: ChannelKey, V: ChannelValue> {
    inner: slotmap::SecondaryMap<K, V>,
    default: V,
}

slotmap::new_key_type! { pub struct RawChannelId; }

pub struct ChannelId<K: ChannelKey, V: ChannelValue> {
    raw: RawChannelId,
    _phantom: PhantomData<(K, V)>,
}
impl<K: ChannelKey, V: ChannelValue> ChannelId<K, V> {
    pub fn new(raw: RawChannelId) -> Self {
        Self {
            raw,
            _phantom: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChannelGroup<K: ChannelKey, V: ChannelValue> {
    channel_names: bimap::BiMap<String, ChannelId<K, V>>,
    channels: SlotMap<RawChannelId, RefCell<Channel<K, V>>>,
}

#[derive(Default, Debug, Clone)]
pub struct MeshChannels {
    channels: HashMap<(ChannelKeyType, ChannelValueType), Box<dyn DynChannelGroup>>,
}

#[derive(Debug, Clone)]
pub struct DefaultChannels {
    pub position: ChannelId<VertexId, Vec3>,
}

impl<K: ChannelKey, V: ChannelValue> std::ops::Index<K> for Channel<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.inner.get(index).unwrap_or(&self.default)
    }
}
impl<K: ChannelKey, V: ChannelValue> std::ops::IndexMut<K> for Channel<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.inner
            .entry(index)
            .expect("Error indexing channel. Key not found")
            .or_default()
    }
}
impl<K: ChannelKey, V: ChannelValue> Channel<K, V> {
    pub fn get(&self, id: K) -> Option<V> {
        self.inner.get(id).copied()
    }
    pub fn get_mut(&mut self, id: K) -> Option<&mut V> {
        Some(self.inner.entry(id)?.or_default())
    }
    pub fn set(&mut self, id: K, val: V) -> Option<()> {
        *self.inner.get_mut(id)? = val;
        Some(())
    }
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.inner.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.inner.iter_mut()
    }
}

impl<K: ChannelKey, V: ChannelValue> ChannelGroup<K, V> {
    pub fn ensure_channel(&mut self, name: &str) -> ChannelId<K, V> {
        match self.channel_names.get_by_left(name) {
            Some(id) => *id,
            None => {
                let ch_id = ChannelId::new(self.channels.insert(Default::default()));
                self.channel_names.insert(name.into(), ch_id);
                ch_id
            }
        }
    }

    pub fn create_channel(&mut self, name: &str) -> Result<ChannelId<K, V>> {
        if self.channel_names.contains_left(name) {
            bail!("The channel named {name} already exists in mesh");
        } else {
            Ok(self.ensure_channel(name))
        }
    }

    pub fn remove_channel(&mut self, id: ChannelId<K, V>) -> Result<Channel<K, V>> {
        self.channel_names.remove_by_right(&id);
        Ok(self
            .channels
            .remove(id.raw)
            .ok_or_else(|| anyhow!("Non-existing channel cannot be removed"))?
            .into_inner())
    }

    pub fn channel_id(&self, name: &str) -> Option<ChannelId<K, V>> {
        self.channel_names.get_by_left(name).copied()
    }

    pub fn channel_name(&self, ch_id: ChannelId<K, V>) -> Option<&str> {
        self.channel_names.get_by_right(&ch_id).map(|x| x.as_str())
    }

    pub fn read_channel(&self, ch_id: ChannelId<K, V>) -> Result<Ref<Channel<K, V>>> {
        self.channels
            .get(ch_id.raw)
            .ok_or_else(|| anyhow!("Channel {ch_id:?} does not exist for this mesh"))?
            .try_borrow()
            .map_err(|err| anyhow!("Channel {ch_id:?} could not be borrowed: {err}"))
    }

    pub fn write_channel(&self, ch_id: ChannelId<K, V>) -> Result<RefMut<Channel<K, V>>> {
        self.channels
            .get(ch_id.raw)
            .ok_or_else(|| anyhow!("Channel {ch_id:?} does not exist for this mesh"))?
            .try_borrow_mut()
            .map_err(|err| anyhow!("Channel {ch_id:?} could not be borrowed: {err}"))
    }
}

impl MeshChannels {
    fn key_of<K: ChannelKey, V: ChannelValue>() -> (ChannelKeyType, ChannelValueType) {
        (K::key_type(), V::value_type())
    }

    fn downcast<K: ChannelKey, V: ChannelValue>(group: &dyn Any) -> &ChannelGroup<K, V> {
        match group.downcast_ref::<ChannelGroup<K, V>>() {
            Some(typed_group) => typed_group,
            None => unreachable!("The invariants of MeshChannels should prevent this."),
        }
    }
    fn downcast_mut<K: ChannelKey, V: ChannelValue>(
        group: &mut dyn Any,
    ) -> &mut ChannelGroup<K, V> {
        match group.downcast_mut::<ChannelGroup<K, V>>() {
            Some(typed_group) => typed_group,
            None => unreachable!("The invariants of MeshChannels should prevent this."),
        }
    }

    fn group<K: ChannelKey, V: ChannelValue>(&self) -> Result<&ChannelGroup<K, V>> {
        Ok(Self::downcast(
            self.channels
                .get(&Self::key_of::<K, V>())
                .ok_or_else(|| anyhow!("There is no channel for {} -> {}", K::name(), V::name()))?
                .as_any(),
        ))
    }

    fn group_mut<K: ChannelKey, V: ChannelValue>(&mut self) -> Result<&mut ChannelGroup<K, V>> {
        Ok(Self::downcast_mut(
            self.channels
                .get_mut(&Self::key_of::<K, V>())
                .ok_or_else(|| anyhow!("There is no channel for {} -> {}", K::name(), V::name()))?
                .as_any_mut(),
        ))
    }

    fn group_or_default<K: ChannelKey, V: ChannelValue>(&mut self) -> &mut ChannelGroup<K, V> {
        Self::downcast_mut(
            self.channels
                .entry(Self::key_of::<K, V>())
                .or_insert_with(|| Box::new(ChannelGroup::<K, V>::default()))
                .as_any_mut(),
        )
    }

    pub fn ensure_channel<K: ChannelKey, V: ChannelValue>(
        &mut self,
        name: &str,
    ) -> ChannelId<K, V> {
        self.group_or_default().ensure_channel(name)
    }

    pub fn create_channel<K: ChannelKey, V: ChannelValue>(
        &mut self,
        name: &str,
    ) -> Result<ChannelId<K, V>> {
        self.group_or_default().create_channel(name)
    }

    pub fn remove_channel<K: ChannelKey, V: ChannelValue>(
        &mut self,
        ch_id: ChannelId<K, V>,
    ) -> Result<Channel<K, V>> {
        self.group_mut()?.remove_channel(ch_id)
    }

    pub fn read_channel<K: ChannelKey, V: ChannelValue>(
        &self,
        ch_id: ChannelId<K, V>,
    ) -> Result<Ref<Channel<K, V>>> {
        self.group()?.read_channel(ch_id)
    }

    pub fn read_channel_by_name<K: ChannelKey, V: ChannelValue>(
        &self,
        name: &str,
    ) -> Result<Ref<Channel<K, V>>> {
        let group = self.group()?;
        group.read_channel(
            group
                .channel_id(name)
                .ok_or_else(|| anyhow!("Channel named {name} does not exist"))?,
        )
    }

    pub fn ensure_group_dyn(
        &mut self,
        kty: ChannelKeyType,
        vty: ChannelValueType,
    ) -> &mut dyn DynChannelGroup {
        type K = ChannelKeyType;
        type V = ChannelValueType;

        macro_rules! ret {
            ($kt:ident, $vt:ident) => {
                self.group_or_default::<$kt, $vt>() as &mut dyn DynChannelGroup
            };
        }

        macro_rules! do_match {
            ($($kt:ident, $vt:ident);*) => {
                match (kty, vty) { $(
                    (K::$kt, V::$vt) => { ret!($kt, $vt) }
                )* }
            }
        }

        do_match! {
            VertexId, Vec2;
            VertexId, Vec3;
            VertexId, Vec4;
            VertexId, f32;
            VertexId, bool;
            FaceId, Vec2;
            FaceId, Vec3;
            FaceId, Vec4;
            FaceId, f32;
            FaceId, bool;
            HalfEdgeId, Vec2;
            HalfEdgeId, Vec3;
            HalfEdgeId, Vec4;
            HalfEdgeId, f32;
            HalfEdgeId, bool
        }
    }

    pub fn ensure_channel_dyn(
        &mut self,
        kty: ChannelKeyType,
        vty: ChannelValueType,
        name: &str,
    ) -> RawChannelId {
        let group = self.ensure_group_dyn(kty, vty);
        group.ensure_channel_dyn(name)
    }

    pub fn dyn_read_channel_by_name(
        &self,
        kty: ChannelKeyType,
        vty: ChannelValueType,
        name: &str,
    ) -> Result<Ref<dyn DynChannel>> {
        let group = self
            .channels
            .get(&(kty, vty))
            .ok_or_else(|| anyhow!("Channel type does not exist"))?;
        let raw_id = group
            .channel_id_dyn(name)
            .ok_or_else(|| anyhow!("Channel value does not exist"))?;
        Ok(group.read_channel_dyn(raw_id))
    }

    pub fn dyn_write_channel_by_name(
        &self,
        kty: ChannelKeyType,
        vty: ChannelValueType,
        name: &str,
    ) -> Result<RefMut<dyn DynChannel>> {
        let group = self
            .channels
            .get(&(kty, vty))
            .ok_or_else(|| anyhow!("Channel type does not exist"))?;
        let raw_id = group
            .channel_id_dyn(name)
            .ok_or_else(|| anyhow!("Channel value does not exist"))?;
        Ok(group.write_channel_dyn(raw_id))
    }

    pub fn write_channel<K: ChannelKey, V: ChannelValue>(
        &self,
        ch_id: ChannelId<K, V>,
    ) -> Result<RefMut<Channel<K, V>>> {
        self.group()?.write_channel(ch_id)
    }

    pub fn write_channel_by_name<K: ChannelKey, V: ChannelValue>(
        &self,
        name: &str,
    ) -> Result<RefMut<Channel<K, V>>> {
        let group = self.group()?;
        group.write_channel(
            group
                .channel_id(name)
                .ok_or_else(|| anyhow!("Channel named {name} does not exist"))?,
        )
    }

    pub fn channel_id<K: ChannelKey, V: ChannelValue>(
        &self,
        name: &str,
    ) -> Option<ChannelId<K, V>> {
        self.group().ok()?.channel_id(name)
    }

    pub fn channel_name<K: ChannelKey, V: ChannelValue>(
        &self,
        ch_id: ChannelId<K, V>,
    ) -> Option<&str> {
        self.group().ok()?.channel_name(ch_id)
    }

    pub fn introspect(
        &self,
    ) -> HashMap<(ChannelKeyType, ChannelValueType), HashMap<String, Vec<String>>> {
        self.channels
            .iter()
            .map(|((k, v), group)| ((*k, *v), group.introspect()))
            .collect()
    }
}

pub trait DynChannel: Any + Debug {
    fn get_lua<'a, 'lua>(&'a self, lua: &'lua mlua::Lua, key: mlua::Value) -> Result<mlua::Value>
    where
        'lua: 'a;
    fn set_lua<'a, 'lua>(
        &'a mut self,
        lua: &'lua mlua::Lua,
        key: mlua::Value,
        value: mlua::Value,
    ) -> Result<()>
    where
        'lua: 'a;
}
impl<K: ChannelKey, V: ChannelValue> DynChannel for Channel<K, V> {
    fn get_lua<'a, 'lua>(&'a self, lua: &'lua mlua::Lua, key: mlua::Value) -> Result<mlua::Value>
    where
        'lua: 'a,
    {
        let key: K = K::cast_from_lua(key, lua)?;
        Ok(self[key].cast_to_lua(lua))
    }

    fn set_lua<'a, 'lua>(
        &'a mut self,
        lua: &'lua mlua::Lua,
        key: mlua::Value,
        value: mlua::Value,
    ) -> Result<()>
    where
        'lua: 'a,
    {
        let key: K = K::cast_from_lua(key, lua)?;
        self[key] = FromToLua::cast_from_lua(value, lua)?;
        Ok(())
    }
}

pub trait DynChannelGroup: Any + Debug + dyn_clone::DynClone {
    fn introspect(&self) -> HashMap<String, Vec<String>>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn ensure_channel_dyn(&mut self, name: &str) -> RawChannelId;
    fn read_channel_dyn(&self, raw_id: RawChannelId) -> Ref<dyn DynChannel>;
    fn write_channel_dyn(&self, raw_id: RawChannelId) -> RefMut<dyn DynChannel>;
    fn channel_id_dyn(&self, name: &str) -> Option<RawChannelId>;
}

impl<K: ChannelKey, V: ChannelValue> DynChannelGroup for ChannelGroup<K, V> {
    fn introspect(&self) -> HashMap<String, Vec<String>> {
        let mut result = HashMap::new();
        for (name, id) in self.channel_names.iter() {
            result.insert(
                name.into(),
                self.read_channel(*id)
                    .unwrap()
                    .iter()
                    .map(|(_k, v)| format!("{:?}", v))
                    .collect(),
            );
        }
        result
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn ensure_channel_dyn(&mut self, name: &str) -> RawChannelId {
        self.ensure_channel(name).raw
    }
    fn read_channel_dyn(&self, raw_id: RawChannelId) -> Ref<dyn DynChannel> {
        self.channels[raw_id].borrow()
    }
    fn write_channel_dyn(&self, raw_id: RawChannelId) -> RefMut<dyn DynChannel> {
        self.channels[raw_id].borrow_mut()
    }
    fn channel_id_dyn(&self, name: &str) -> Option<RawChannelId> {
        self.channel_names.get_by_left(name).map(|x| x.raw)
    }
}

impl DefaultChannels {
    pub fn with_position(channels: &mut MeshChannels) -> Self {
        let position = channels.ensure_channel::<VertexId, Vec3>("position".into());
        Self { position }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_channels() {
        let mut vertices: slotmap::SlotMap<VertexId, ()> = slotmap::SlotMap::with_key();
        let v1 = vertices.insert(());
        let v2 = vertices.insert(());
        let v3 = vertices.insert(());

        let mut mesh_channels = MeshChannels::default();
        let position = mesh_channels
            .create_channel::<VertexId, Vec3>("position".into())
            .unwrap();
        let color = mesh_channels
            .create_channel::<VertexId, Vec4>("color".into())
            .unwrap();
        let size = mesh_channels
            .create_channel::<VertexId, f32>("size".into())
            .unwrap();

        assert!(mesh_channels.channel_id("position").unwrap() == position);
        assert!(mesh_channels.channel_id("color").unwrap() == color);
        assert!(mesh_channels.channel_id("size").unwrap() == size);

        {
            let mut positions = mesh_channels.write_channel(position).unwrap();
            let mut colors = mesh_channels.write_channel(color).unwrap();
            let mut sizes = mesh_channels.write_channel(size).unwrap();

            positions[v1] = Vec3::X;
            positions[v2] = Vec3::Y;
            positions[v3] = Vec3::Z;

            colors[v1] = Vec4::splat(0.0);
            colors[v2] = Vec4::splat(0.5);
            colors[v3] = Vec4::splat(1.0);

            sizes[v1] = 0.25;
            sizes[v2] = 0.50;
            sizes[v3] = 1.0;

            // Re-borrowing the position channel should fail now
            assert!(mesh_channels.read_channel(position).is_err());
        }

        {
            let positions = mesh_channels.read_channel(position).unwrap();
            let colors = mesh_channels.read_channel(color).unwrap();
            let sizes = mesh_channels.read_channel(size).unwrap();

            assert_eq!(positions[v1], Vec3::X);
            assert_eq!(positions[v2], Vec3::Y);
            assert_eq!(positions[v3], Vec3::Z);

            assert_eq!(colors[v1], Vec4::splat(0.0));
            assert_eq!(colors[v2], Vec4::splat(0.5));
            assert_eq!(colors[v3], Vec4::splat(1.0));

            assert_eq!(sizes[v1], 0.25);
            assert_eq!(sizes[v2], 0.50);
            assert_eq!(sizes[v3], 1.0);

            // Re-reading a channel works, because we only hold Refs
            assert!(mesh_channels.read_channel(position).is_ok());
            // But trying to write still fails
            assert!(mesh_channels.write_channel(position).is_err());
        }

        // Once the refs are dropped, we can write again
        assert!(mesh_channels.write_channel(position).is_ok());

        // The introspection API can be used to inspect the existing channels
        // without necessarily knowing which channels are registered or their
        // types.
        let introspected = mesh_channels.introspect();
        assert_eq!(
            &introspected[&(ChannelKeyType::VertexId, ChannelValueType::Vec4)]["color"],
            &[
                "Vec4(0.0, 0.0, 0.0, 0.0)",
                "Vec4(0.5, 0.5, 0.5, 0.5)",
                "Vec4(1.0, 1.0, 1.0, 1.0)",
            ]
        );
        assert_eq!(
            &introspected[&(ChannelKeyType::VertexId, ChannelValueType::f32)]["size"],
            &["0.25", "0.5", "1.0",]
        );
        assert_eq!(
            &introspected[&(ChannelKeyType::VertexId, ChannelValueType::Vec3)]["position"],
            &[
                "Vec3(1.0, 0.0, 0.0)",
                "Vec3(0.0, 1.0, 0.0)",
                "Vec3(0.0, 0.0, 1.0)",
            ]
        );

        // Channels can also be read and written using a type-erased API. This
        // is mainly used for interfacing with Lua and looks very clunky here.
        // When programming in Rust, using the type-safe API is preferred
        let lua = Lua::new();
        let dyn_pos = mesh_channels
            .dyn_read_channel_by_name(ChannelKeyType::VertexId, ChannelValueType::f32, "size")
            .unwrap();
        match dyn_pos.get_lua(&lua, v1.cast_to_lua(&lua)).unwrap() {
            mlua::Value::Number(x) if x == 0.25 => {}
            _ => panic!("Expected the number 0.25"),
        }
        drop(dyn_pos);
    }

    #[test]
    pub fn test_ensure_channel() {
        let mut mesh_channels = MeshChannels::default();

        let position = mesh_channels
            .create_channel::<VertexId, Vec3>("position")
            .unwrap();
        assert_eq!(
            position,
            mesh_channels.ensure_channel::<VertexId, Vec3>("position")
        );
    }
}

// ------------- Boilerplate zone ------------

// NOTE: Slotmap requires a bunch of traits that we can't derive on our
// ChannelKey type because it's generic and has a PhantomData, which rust's std
// derives can't handle. A crate like `derivative` could be used here, but the
// extra dependency for a single usage is not justified.

impl<K: ChannelKey, V: ChannelValue> Clone for ChannelId<K, V> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw,
            _phantom: self._phantom,
        }
    }
}
impl<K: ChannelKey, V: ChannelValue> Copy for ChannelId<K, V> {}
impl<K: ChannelKey, V: ChannelValue> Default for ChannelId<K, V> {
    fn default() -> Self {
        Self {
            raw: Default::default(),
            _phantom: Default::default(),
        }
    }
}
impl<K: ChannelKey, V: ChannelValue> PartialEq for ChannelId<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self._phantom == other._phantom
    }
}
impl<K: ChannelKey, V: ChannelValue> Eq for ChannelId<K, V> {}
impl<K: ChannelKey, V: ChannelValue> Ord for ChannelId<K, V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}
impl<K: ChannelKey, V: ChannelValue> PartialOrd for ChannelId<K, V> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.raw.partial_cmp(&other.raw)
    }
}
impl<K: ChannelKey, V: ChannelValue> std::hash::Hash for ChannelId<K, V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}
impl<K: ChannelKey, V: ChannelValue> Debug for ChannelId<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelId")
            .field("data", &self.raw)
            .finish()
    }
}

impl<K: ChannelKey, V: ChannelValue> Default for Channel<K, V> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            default: Default::default(),
        }
    }
}

impl<K: ChannelKey, V: ChannelValue> Default for ChannelGroup<K, V> {
    fn default() -> Self {
        Self {
            channel_names: Default::default(),
            channels: Default::default(),
        }
    }
}

dyn_clone::clone_trait_object!(DynChannelGroup);
