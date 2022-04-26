use std::borrow::Cow;

use super::*;

pub struct LuaSourceFile {
    contents: String,
    name: String,
}
impl<'lua> AsChunk<'lua> for LuaSourceFile {
    fn source(&self) -> std::result::Result<Cow<'_, [u8]>, std::io::Error> {
        self.contents.source()
    }

    fn name(&self) -> std::option::Option<String> {
        Some(self.name.to_string())
    }
}

pub fn load(lua: &Lua) -> anyhow::Result<NodeDefinitions> {
    for entry in walkdir::WalkDir::new("node_libraries")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let is_lua_file = entry.file_type().is_file()
            && entry
                .file_name()
                .to_str()
                .map(|s| s.ends_with(".lua"))
                .unwrap_or(false);

        if is_lua_file {
            let path = entry.path();

            let path_display = format!("{}", path.display());

            println!("Loading Lua file {}", path_display);

            lua.load(&LuaSourceFile {
                contents: std::fs::read_to_string(path).unwrap_or_else(|err| {
                    format!("error('Error reading file \"{:?}\". {}')", path, err)
                }),
                name: path_display,
            })
            .exec()?;
        }
    }

    let table = lua
        .globals()
        .get::<_, Table>("NodeLibrary")?
        .get::<_, Table>("nodes")?;
    NodeDefinition::load_nodes_from_table(table)
}
