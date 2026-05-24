use std::ops::BitOr;
#[derive(Eq, PartialEq, Clone, Copy, PartialOrd, Ord)]
#[repr(transparent)]
pub struct SecurityContext(u8);
impl SecurityContext {
    pub const NONE: SecurityContext = SecurityContext(0);
    pub const PLUGIN: SecurityContext = SecurityContext(0x1);
    pub const CORE_PLACE: SecurityContext = SecurityContext(0x2);
    pub const WRITE_PLAYER: SecurityContext = SecurityContext(0x4);
    pub const LOCAL_USER: SecurityContext = SecurityContext(0x10);
    pub const CORE_SCRIPT: SecurityContext = SecurityContext(0x1D);
    pub const CORE: SecurityContext = SecurityContext(0x1F);
    pub const TEST_LOCAL_USER: SecurityContext = SecurityContext(0x10);
}

impl std::fmt::Debug for SecurityContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::NONE => f.write_str("SecurityContext::NONE"),
            Self::PLUGIN => f.write_str("SecurityContext::PLUGIN"),
            Self::CORE_PLACE => f.write_str("SecurityContext::CORE_PLACE"),
            Self::WRITE_PLAYER => f.write_str("SecurityContext::WRITE_PLAYER"),
            Self::LOCAL_USER => f.write_str("SecurityContext::LOCAL_USER"),
            Self::CORE_SCRIPT => f.write_str("SecurityContext::CORE_SCRIPT"),
            Self::CORE => f.write_str("SecurityContext::CORE"),
            _ => f.debug_tuple("SecurityContext").field(&self.0).finish(),
        }
    }
}

impl std::fmt::Display for SecurityContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::NONE => f.write_str("None"),
            Self::PLUGIN => f.write_str("Plugin"),
            Self::CORE_PLACE => f.write_str("CorePlace"),
            Self::WRITE_PLAYER => f.write_str("WritePlayer"),
            Self::LOCAL_USER => f.write_str("LocalUser"),
            Self::CORE_SCRIPT => f.write_str("CoreScript"),
            Self::CORE => f.write_str("Core"),
            _ => write!(f, "0x{:x}", self.0),
        }
    }
}

impl BitOr for SecurityContext {
    type Output = SecurityContext;
    fn bitor(self, rhs: Self) -> Self::Output {
        SecurityContext(self.0 | rhs.0)
    }
}

impl SecurityContext {
    pub const fn has(self, other: SecurityContext) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl Into<u8> for SecurityContext {
    fn into(self) -> u8 {
        self.0
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThreadIdentityType {
    Anon,
    /// UserInit is a thread that was created by a user, such as a plugin or a command bar script.
    UserInit,
    /// Script is a thread that was created by a script.
    Script,
    /// Script in a core place.
    ScriptInCorePlace,
    /// Core script
    CoreScript,
    /// Command bar script
    StudioCommandBar,
    /// Studio plugin
    StudioPlugin,
    /// Web server
    WebServer,
    /// Replication from server to client
    Replication,
}

impl Default for ThreadIdentityType {
    fn default() -> Self {
        ThreadIdentityType::Anon
    }
}
impl ThreadIdentityType {
    #[inline]
    pub const fn get_security_contexts(&self) -> SecurityContext {
        match self {
            ThreadIdentityType::Anon => SecurityContext::NONE,
            ThreadIdentityType::UserInit => SecurityContext(
                SecurityContext::PLUGIN.0
                    | SecurityContext::CORE_PLACE.0
                    | SecurityContext::LOCAL_USER.0,
            ),
            ThreadIdentityType::Script => SecurityContext::NONE,
            ThreadIdentityType::ScriptInCorePlace => SecurityContext::CORE_PLACE,
            ThreadIdentityType::CoreScript => SecurityContext(
                SecurityContext::PLUGIN.0
                    | SecurityContext::CORE_PLACE.0
                    | SecurityContext::LOCAL_USER.0
                    | SecurityContext::CORE_SCRIPT.0,
            ),
            ThreadIdentityType::StudioCommandBar => SecurityContext(
                SecurityContext::PLUGIN.0
                    | SecurityContext::CORE_PLACE.0
                    | SecurityContext::LOCAL_USER.0,
            ),
            ThreadIdentityType::StudioPlugin => SecurityContext::PLUGIN,
            ThreadIdentityType::WebServer => SecurityContext::CORE,
            ThreadIdentityType::Replication => SecurityContext(
                SecurityContext::WRITE_PLAYER.0
                    | SecurityContext::CORE_PLACE.0
                    | SecurityContext::CORE_SCRIPT.0,
            ),
        }
    }
}
