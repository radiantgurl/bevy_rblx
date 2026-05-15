use mlua::Lua;

use crate::{
    core::{LuauContainer, ThreadIdentity, ThreadIdentityType},
    internal_prelude::*,
};

#[test]
pub fn test_identities() {
    let luau = LuauContainer::default();
    unsafe {
        ThreadIdentity::set(
            &luau.lua,
            ThreadIdentity {
                identity: ThreadIdentityType::CoreScript,
                script: None,
            },
        )
    }

    // let ptr = std::ptr::null()
    assert_eq!(
        ThreadIdentity::fetch(&luau.lua).identity,
        ThreadIdentityType::CoreScript,
        "a thread must keep its identity"
    );
    let thr = luau
        .lua
        .create_thread(
            luau.lua
                .create_function(move |l: &Lua, ()| {
                    println!("running subthread identity test");
                    assert_eq!(
                        ThreadIdentity::fetch(l).identity,
                        ThreadIdentityType::CoreScript,
                        "a thread must inherit its identity"
                    );
                    Ok(())
                })
                .unwrap(),
        )
        .unwrap();
    ThreadIdentity::erase_thr(&luau.lua, luau.lua.to_pointer() as usize);
    assert_eq!(
        ThreadIdentity::fetch(&luau.lua).identity,
        ThreadIdentityType::Anon,
        "a thread can have its identity erased"
    );
    thr.resume::<()>(()).unwrap();
}
