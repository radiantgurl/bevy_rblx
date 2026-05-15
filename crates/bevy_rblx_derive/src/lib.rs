#![feature(if_let_guard)]

use parse::AttrArguments;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    Error, Expr, ExprArray, Ident, ItemEnum, ItemImpl, LitStr, Token, Type, parse::Parse, parse_macro_input, spanned::Spanned
};
use utils::camel_case_to_snake_case;

use crate::{parse::ClassArgs, utils::snake_case_to_camel_case};

mod parse;
mod utils;

#[proc_macro_attribute]
pub fn lua_enum(
    arguments: proc_macro::TokenStream,
    ts: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let enum_block: ItemEnum = parse_macro_input!(ts);
    let name = enum_block.ident.clone();
    let vis = enum_block.vis.clone();

    let args: AttrArguments = parse_macro_input!(arguments);

    let default_impl = if let Some(default) = args.get_named_arg("default") {
        let default_val = &default.value;
        let default_name: Ident = {
            let default_name_arg = args.get_named_arg("default_name").map(|x| x.value.clone());
            match default_name_arg {
                Some(x) => match x.try_into() {
                    Ok(x) => x,
                    Err(e) => {
                        return e.into_compile_error().into();
                    }
                },
                None => Ident::new("Default", Span::call_site()),
            }
        };
        quote! {
            impl Default for #name {
                fn default() -> Self {
                    Self::#default_name
                }
            }

            impl #name {
                pub const fn collapse_default(self) -> Self {
                    match self {
                        Self::#default_name => Self::#default_val,
                        x => x
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    let mut last_index: Option<usize> = None;
    let mut variants = vec![];

    for x in enum_block.variants.iter() {
        match x.discriminant.as_ref().map(|(_, expr)| expr) {
            Some(e)
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(i),
                    ..
                }) = e =>
            {
                last_index = Some(i.base10_parse().unwrap());
                variants.push((x.ident.to_string(), last_index.unwrap() as i16));
            }
            None => match last_index {
                Some(i) => {
                    last_index = Some(i + 1);
                    variants.push((x.ident.to_string(), last_index.unwrap() as i16));
                }
                None => {
                    last_index = Some(0);
                    variants.push((x.ident.to_string(), 0));
                }
            },
            _ => {
                return syn::Error::new(x.span(), "invalid discriminant")
                    .into_compile_error()
                    .into();
            }
        }
    }

    let variant_quotes_names: Vec<proc_macro2::TokenStream> = variants
        .iter()
        .map(|(variant_name, _value)| {
            let variant_name = Ident::new(variant_name, Span::call_site());
            quote! {
                Self::#variant_name => concat!(stringify!(#name), ".", stringify!(#variant_name))
            }
        })
        .collect();
    let variant_quotes_names_only: Vec<proc_macro2::TokenStream> = variants
        .iter()
        .map(|(variant_name, _value)| {
            let variant_name = Ident::new(variant_name, Span::call_site());
            quote! {
                Self::#variant_name => stringify!(#variant_name)
            }
        })
        .collect();

    let variant_fields: Vec<proc_macro2::TokenStream> = variants
        .iter()
        .map(|(variant_name, _value)| {
            let variant_name = Ident::new(variant_name, Span::call_site());
            quote! {
                fields.add_field(stringify!(#variant_name), #name::#variant_name);
            }
        })
        .collect();

    let enum_type_name = Ident::new(&format!("LuaEnum{}", name.to_string()), Span::call_site());

    quote! {
        use mlua::prelude::*;

        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
        #[repr(i16)]
        #enum_block

        impl FromLua for #name {
            fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
                let ud = value.as_userdata();
                if ud.is_none() {
                    Err(LuaError::FromLuaConversionError {
                        from: value.type_name(),
                        to: "EnumItem".into(),
                        message: None,
                    })
                } else {
                    let unwrapped = unsafe { ud.unwrap_unchecked() }.borrow::<Self>();
                    if unwrapped.is_err() {
                        Err(LuaError::FromLuaConversionError {
                            from: "userdata",
                            to: "EnumItem".into(),
                            message: None,
                        })
                    } else {
                        unsafe { Ok(*unwrapped.unwrap_unchecked()) }
                    }
                }
            }
        }

        impl LuaUserData for #name {
            fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
                methods.add_meta_method("__tostring", |_, this, ()| {
                    Ok(String::from(match *this {
                        #(#variant_quotes_names),*
                    }))
                });
            }
            fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
                fields.add_meta_field("__type", "EnumItem");
                fields.add_field_method_get("Name", |_, this| Ok(match (this) {
                    #(#variant_quotes_names_only),*
                }));
                fields.add_field_method_get("Value", |_, this| Ok(*this as i16));

            }
        }

        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
        #vis struct #enum_type_name;

        impl FromLua for #enum_type_name {
            fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
                let ud = value.as_userdata();
                if ud.is_none() {
                    Err(LuaError::FromLuaConversionError {
                        from: value.type_name(),
                        to: "Enum".into(),
                        message: None,
                    })
                } else {
                    let unwrapped =
                        unsafe { ud.unwrap_unchecked() }.borrow::<Self>();
                    if unwrapped.is_err() {
                        Err(LuaError::FromLuaConversionError {
                            from: "userdata",
                            to: "Enum".into(),
                            message: None,
                        })
                    } else {
                        unsafe { Ok(*unwrapped.unwrap_unchecked()) }
                    }
                }
            }
        }

        impl LuaUserData for #enum_type_name {
            fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
                methods.add_meta_method("__tostring", |_, _, ()| {
                    Ok(String::from(concat!("Enums.",stringify!(#name))))
                });
            }
            fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
                fields.add_meta_field("__type", "Enum");
                #(#variant_fields)*
            }
        }

        #default_impl
    }
    .into()
}

#[doc(hidden)]
#[proc_macro]
pub fn create_enums(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let wrapper: ExprArray = parse_macro_input!(ts);
    let mut enums: Vec<Ident> = vec![];
    for token in wrapper.elems.iter() {
        if let syn::Expr::Path(p) = token {
            if let Some(i) = p.path.get_ident() {
                enums.push(i.clone());
            } else {
                return syn::Error::new(p.path.span(), "not an ident")
                    .into_compile_error()
                    .into();
            }
        } else {
            return syn::Error::new(token.span(), "not a path")
                .into_compile_error()
                .into();
        }
    }

    let modules: Vec<proc_macro2::TokenStream> = enums
        .iter()
        .map(|x| {
            let ident = Ident::new(&camel_case_to_snake_case(x.to_string().as_str()), x.span());
            quote! {
                mod #ident;
            }
        })
        .collect();

    let enum_use: Vec<proc_macro2::TokenStream> = enums
        .iter()
        .map(|x| {
            let ident = Ident::new(&camel_case_to_snake_case(x.to_string().as_str()), x.span());
            let enum_type_name =
                Ident::new(&format!("LuaEnum{}", x.to_string()), Span::call_site());
            quote! {
                pub use #ident::#x;
                use #ident::#enum_type_name;
            }
        })
        .collect();

    let enum_fields: Vec<proc_macro2::TokenStream> = enums
        .iter()
        .map(|x| {
            let enum_type_name =
                Ident::new(&format!("LuaEnum{}", x.to_string()), Span::call_site());
            quote! {
                fields.add_field(stringify!(#x), #enum_type_name);
            }
        })
        .collect();

    quote! {
        use mlua::prelude::*;
        use bevy_rblx_derive::register;

        #(#modules)*
        #(#enum_use)*

        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
        pub struct LuaEnums;

        impl FromLua for LuaEnums {
            fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
                let ud = value.as_userdata();
                if ud.is_none() {
                    Err(LuaError::FromLuaConversionError {
                        from: value.type_name(),
                        to: "Enums".into(),
                        message: None,
                    })
                } else {
                    let unwrapped = unsafe { ud.unwrap_unchecked() }.borrow::<LuaEnums>();
                    if unwrapped.is_err() {
                        Err(LuaError::FromLuaConversionError {
                            from: "userdata",
                            to: "Enums".into(),
                            message: None,
                        })
                    } else {
                        unsafe { Ok(*unwrapped.unwrap_unchecked()) }
                    }
                }
            }
        }

        impl LuaUserData for LuaEnums {
            fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
                methods.add_meta_method("__tostring", |_, _, ()| {
                    Ok(String::from("Enums"))
                });
            }
            fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
                fields.add_meta_field("__type", "Enums");
                #(#enum_fields)*
            }
        }
        #[register]
        impl bevy_rblx::core::LuaSingleton for LuaEnums {
            fn register_singleton(lua: &Lua) -> LuaResult<()> {
                lua.globals().raw_set("Enums", LuaEnums)?;
                Ok(())
            }
        }

    }
    .into()
}

#[proc_macro_attribute]
pub fn register(
    arguments: proc_macro::TokenStream,
    ts: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if !arguments.is_empty() {
        let args_2: TokenStream = arguments.into();
        return syn::Error::new(args_2.span(), "expected ]")
            .into_compile_error()
            .into();
    }
    let impl_block = parse_macro_input!(ts as ItemImpl);
    let name = &impl_block.self_ty;
    if impl_block.trait_.is_none() {
        return Error::new_spanned(impl_block, "expected LuaSingleton impl block")
            .into_compile_error()
            .into();
    }
    quote! {
        #impl_block
        inventory::submit!(
            bevy_rblx::core::singleton::SingletonRegisterFn(#name::register_singleton)
        );
    }
    .into()
}

struct FastFlagArgs {
    name: Ident,
    _colon: Token![:],
    ty: Type,
    _eq: Token![=],
    expr: Expr,
}

impl Parse for FastFlagArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(FastFlagArgs {
            name: input.parse()?,
            _colon: input.parse()?,
            ty: input.parse()?,
            _eq: input.parse()?,
            expr: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn fast_flag(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let FastFlagArgs { name, ty, expr, .. } = parse_macro_input!(ts as FastFlagArgs);

    quote! {
        #[derive(Clone, Copy, Hash, Debug)]
        pub struct #name;
        const _: () = {
            use ::core::sync::atomic::{AtomicUsize, Ordering};
            static ID: AtomicUsize = AtomicUsize::new(0);

            impl bevy_rblx::core::FastFlagKey for #name {
                type Target = #ty;
                const NAME: &'static str = stringify!(#name);

                fn fetch_internal_id() -> usize {
                    ID.load(Ordering::Relaxed)
                }
                fn default_value() -> Self::Target {
                    #expr
                }
                unsafe fn set_internal_id(id: usize) {
                    ID.store(id, Ordering::Relaxed)
                }
            }
            fn register_fastflag(ff: &mut bevy_rblx::internal::FastFlagKeyInserter) {
                ff.insert_key::<#name>();
            }
            bevy_rblx::internal::inventory::submit!(bevy_rblx::internal::FastFlagKeyInsert(register_fastflag));
        };
    }.into()
}

#[proc_macro]
pub fn register_class(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = parse_macro_input!(ts as ClassArgs);

    let class_name = args.class_name;
    let class_name_members_ident = Ident::new(&format!("{class_name}Members"), class_name.span());
    let inherits = args.inherits.iter();
    let inherits_members = args
        .inherits
        .iter()
        .map(|x| Ident::new(&format!("{x}Members"), x.span()))
        .filter(|i| i != "ObjectMembers")
        .map(|i| {
            let mut punctuated = syn::punctuated::Punctuated::new();

            punctuated.push(syn::PathSegment {
                ident: i,
                arguments: syn::PathArguments::None,
            });
            Type::Path(syn::TypePath {
                qself: None,
                path: syn::Path {
                    leading_colon: None,
                    segments: punctuated,
                },
            })
        })
        .chain(
            args.require_components
                .as_ref()
                .cloned()
                .unwrap_or_else(|| syn::punctuated::Punctuated::new()),
        );

    let vtable_name = Ident::new(
        &format!("{}_VTABLE", class_name.to_string().to_ascii_uppercase()),
        class_name.span(),
    );

    let members = {
        let struct_spanned = {
            let derive = quote_spanned! { args.members_token.span() =>
                #[derive(Clone, bevy::prelude::Component)]
            };
            let head = quote_spanned! {args.members_token.span() =>
                pub struct #class_name_members_ident
            };
            quote! {
                #derive
                #[require(#(#inherits_members),*)]
                #head
            }
        };

        let impl_header = quote_spanned! { args.members_token.span() =>
            impl #class_name
        };

        let raw_fields = args
            .members
            .fields
            .iter()
            .filter(|x| x.r#virtual.is_none())
            .map(|field| {
                let vis = &field.visibility;
                let name = &field.name;
                let ty = &field.ty;
                quote! {
                    #vis #name: #ty
                }
            });

        let impl_default = {
            let impl_default_fields =
                args.members
                    .fields
                    .iter()
                    .filter(|x| x.r#virtual.is_none())
                    .map(|field| {
                        let name = &field.name;
                        let default =
                            field.default.as_ref().map(|x| quote! {#x}).unwrap_or_else(
                                || quote_spanned! {name.span() => Default::default()},
                            );
                        quote! {
                            #name: #default
                        }
                    });

            quote! {
                impl Default for #class_name_members_ident {
                    fn default() -> Self {
                        Self {
                            #(#impl_default_fields),*
                        }
                    }
                }
            }
        };

        let getters_setters = args.members.fields.iter().filter(|x| x.r#priv.is_none()).map(|field| {
            let name = &field.name;
            let get_name = Ident::new(&format!("get_{name}"), name.span());
            let set_name = Ident::new(&format!("set_{name}"), name.span());
            let getter = if let Some((lua_method_closure, block)) = field.getter.as_ref() {
                let args = &lua_method_closure.args;
                let async_kw = &lua_method_closure.async_token;
                let lua_arg = &lua_method_closure.lua_arg;
                let self_arg = {
                    let self_name = &lua_method_closure.self_name;
                    let self_ty_span = lua_method_closure.self_ty.clone();
                    let self_ty = quote_spanned! {self_ty_span.span() => bevy_rblx::internal::#self_ty_span};
                    quote! {
                        #self_name: #self_ty
                    }
                };
                let lua_res = &lua_method_closure.lua_result;
                let lt = &lua_method_closure.lt;
                let gt = &lua_method_closure.gt;
                let return_type = &lua_method_closure.return_type;
                quote! {
                    pub #async_kw fn #get_name(#lua_arg, #self_arg #args) -> #lua_res #lt #return_type #gt #block
                }
            } else {
                let ty = &field.ty;
                let field_name = &field.name;
                let code_block = quote_spanned! { ty.span() =>
                    let world_access = bevy_rblx::internal::WorldAccess::fetch_readonly(lua);
                    let world = world_access.access_read_only();
                    world.get::<#class_name_members_ident>(this).expect("object has members struct").#field_name.clone().into_lua(lua)
                };
                quote_spanned! { get_name.span() =>
                    pub fn #get_name(lua: &Lua, this: bevy_rblx::internal::Entity, _vtable: &'static bevy_rblx::internal::ObjectVTable) -> LuaResult<LuaValue> {
                        #code_block
                    }
                }
            };
            let setter = if let Some((lua_method_closure, block)) = field.setter.as_ref() {
                let args = &lua_method_closure.args;
                let async_kw = &lua_method_closure.async_token;
                let lua_arg = &lua_method_closure.lua_arg;
                let self_arg = {
                    let self_name = &lua_method_closure.self_name;
                    let self_ty_span = lua_method_closure.self_ty.clone();
                    let self_ty = quote_spanned! {self_ty_span.span() => bevy_rblx::internal::#self_ty_span};
                    quote! {
                        #self_name: #self_ty
                    }
                };
                let lua_res = &lua_method_closure.lua_result;
                let lt = &lua_method_closure.lt;
                let gt = &lua_method_closure.gt;
                let return_type = &lua_method_closure.return_type;
                quote! {
                    pub #async_kw fn #set_name(#lua_arg, #self_arg #args) -> #lua_res #lt #return_type #gt #block
                }
            } else if field.read_only.is_none() && field.r#virtual.is_none() {
                let ty = &field.ty;
                let field_name = &field.name;
                let code_block = quote_spanned! { ty.span() =>
                    let mut world_access = bevy_rblx::internal::WorldAccess::fetch(lua);
                    let world = world_access.access_synchronized()?;
                    let field = &mut world.get_mut::<#class_name_members_ident>(this).expect("object has members struct").#field_name;
                    let new_field: #ty = bevy_rblx::internal::FromLua::from_lua(new_value, lua)?;
                    let diff = *field != new_field;
                    if diff {
                        *field = new_field;
                    }
                    Ok(diff)
                };
                quote_spanned! { get_name.span() =>
                    pub fn #set_name(lua: &Lua, this: bevy_rblx::internal::Entity, _vtable: &'static bevy_rblx::internal::ObjectVTable, new_value: LuaValue) -> LuaResult<bool> {
                        #code_block
                    }
                }
            } else {
                quote! {}
            };
            quote! {
                #getter
                #setter
            }
        }).filter(|x| !x.is_empty());

        let impl_member_fetchers = {
            let new_lit_str = LitStr::new(&format!("expected {class_name}"), class_name.span());
            quote_spanned! {class_name.span() =>
                impl #class_name_members_ident {
                    pub fn fetch_members<'a>(world: &'a ::bevy::prelude::World, this: ::bevy::prelude::Entity) -> &'a Self {
                        world.get::<#class_name_members_ident>(this).expect(#new_lit_str)
                    }
                    pub fn fetch_members_mut<'a>(world: &'a mut ::bevy::prelude::World, this: ::bevy::prelude::Entity) -> ::bevy::prelude::Mut<'a, Self> {
                        world.get_mut::<#class_name_members_ident>(this).expect(#new_lit_str)
                    }
                }
            }
        };
        quote! {
            #impl_header
            {
                #(#getters_setters)*
            }
            #struct_spanned
            {
                #(#raw_fields),*
            }
            #impl_default
            #impl_member_fetchers
        }
    };

    let property_infos = {
        args.members
            .fields
            .iter()
            .filter(|f| f.r#priv.is_none())
            .map(|field| {
                let name = &field.name;
                let renamed = field.rename.as_ref().cloned().unwrap_or_else(|| {
                    syn::LitStr::new(&snake_case_to_camel_case(&name.to_string()), name.span())
                });
                let security_context = field
                    .security
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| Ident::new("NONE", name.span()));
                let get_name = Ident::new(&format!("get_{name}"), name.span());
                let set_name = Ident::new(&format!("set_{name}"), name.span());

                let setter = if field.read_only.is_some()
                    || (field.setter.is_none() && field.r#virtual.is_some())
                {
                    quote_spanned! {field.read_only.span() => None}
                } else {
                    quote! {
                        Some(#class_name::#set_name)
                    }
                };
                let final_quote = quote! {
                    bevy_rblx::internal::ObjectPropertyInfo {
                        property_name: #renamed,
                        security: bevy_rblx::internal::SecurityContext::#security_context,

                        getter: #class_name::#get_name,
                        setter: #setter
                    }
                };
                if field.deprecated_aliases.is_empty() {
                    final_quote
                } else {
                    let mut quotes = quote! {};

                    for i in field.deprecated_aliases.iter() {
                        quotes = quote! {
                            #quotes
                            #[cfg(feature="deprecated")]
                            bevy_rblx::internal::ObjectPropertyInfo {
                                property_name: #i,
                                security: bevy_rblx::internal::SecurityContext::#security_context,

                                getter: #class_name::#get_name,
                                setter: #setter
                            },
                        }
                    }
                    quote! {
                        #quotes
                        #final_quote
                    }
                }
            })
    };

    let methods = {
        let processed_methods = args.methods.0.iter().map(|method_info| {
            let parse::Method { sign: parse::LuaMethod { async_token, name, lua_arg, self_name, self_ty, args: args_1, lua_result, lt, return_type, gt }, code, .. } = method_info;
            let disassembled_code = &code.block;
            let security_guard = if let Some(security) = &method_info.meta.security {
                quote_spanned! { security.span() =>
                    {
                        let current_context = bevy_rblx::internal::ThreadIdentity::fetch(lua).identity.get_security_contexts();
                        let expected_context = bevy_rblx::internal::SecurityContext::#security;
                        if !current_context.has(expected_context) {
                            return Err(bevy_rblx::internal::LuaError::runtime(format!(
                                "thread with security context {current_context} is missing {expected_context}"
                            )));
                        }
                    }
                }
            } else {
                quote! {}
            };
            let is_a_check = quote! {
                {
                    let world_access = bevy_rblx::internal::WorldAccess::fetch(lua);
                    let world = world_access.access_read_only();
                    if !world.get::<bevy_rblx::internal::ObjectHeader>(#self_name.entity()).expect("entity is object").vtable.method_resolution_order.iter().any(|v| v.class_name == stringify!(#class_name)) {
                        return Err(bevy_rblx::internal::LuaError::runtime(concat!("object is not ", stringify!(#class_name))));
                    }
                }
            };

            let args_generated = args_1.add_self_type(self_name, self_ty);

            quote! {
                pub #async_token fn #name(#lua_arg #args_generated) -> #lua_result #lt #return_type #gt {
                    #security_guard
                    #is_a_check
                    #disassembled_code
                }
            }
        });
        let header = quote_spanned! { args.methods_token.span() =>
            impl #class_name
        };
        quote! {
            #header
            {
                #(#processed_methods)*
            }
        }
    };

    let method_infos = {
        args.methods.0.iter().map(|parse::Method { meta: parse::MethodMeta { rename, security, deprecated_aliases, .. }, sign: parse::LuaMethod { name, .. }, ..}| {
            let actual_name = if rename.is_some() {
                quote! {
                    #rename
                }
            } else {
                let new_name = syn::LitStr::new(&snake_case_to_camel_case(&name.to_string()), name.span());
                quote! {
                    #new_name
                }
            };
            let security = security.as_ref().cloned().unwrap_or_else(|| Ident::new("NONE", name.span()));
            let mut deprecated_quotes = quote!{};

            for i in deprecated_aliases {
                deprecated_quotes = quote!{
                    #deprecated_quotes
                    bevy_rblx::internal::ObjectMethodInfo {
                        method_name: #i,
                        security: bevy_rblx::internal::SecurityContext::#security,

                        function: bevy_rblx::internal::CachedLuaFunction::new(move |l: &Lua| l.create_function(#class_name::#name).expect("function creation shouldnt error"))
                    },
                }
            }
            quote! {
                #deprecated_quotes
                bevy_rblx::internal::ObjectMethodInfo {
                    method_name: #actual_name,
                    security: bevy_rblx::internal::SecurityContext::#security,

                    function: bevy_rblx::internal::CachedLuaFunction::new(move |l: &Lua| l.create_function(#class_name::#name).expect("function creation shouldnt error"))
                }
            }
        })
    };

    let new_fn_define = if args.abstract_token.is_some() {
        quote! {}
    } else if let Some(custom) = &args.custom_constructor {
        let parse::NewFn {
            lua_arg,
            entity_command_ident,
            entity_command_type,
            lua_result,
            lt,
            return_type,
            gt,
            code,
        } = custom;
        quote! {
            impl #class_name {
                fn constructor(#lua_arg, mut #entity_command_ident: #entity_command_type) -> #lua_result #lt #return_type #gt #code
            }
        }
    } else {
        quote_spanned! { class_name.span() =>
            impl #class_name {
                fn constructor(_lua: &Lua, mut entity_commands: bevy_rblx::internal::EntityCommands) -> bevy_rblx::internal::LuaResult<()> {
                    entity_commands.insert(#class_name_members_ident::default());
                    entity_commands.insert(bevy_rblx::internal::ObjectHeader::new(bevy_rblx::internal::OBJECT_VTABLES.get(stringify!(#class_name)).unwrap()));
                    Ok(())
                }
            }
        }
    };

    let new_fn = if let Some(abstract_token) = args.abstract_token.as_ref() {
        quote_spanned! {abstract_token.span() => None}
    } else if let Some(priv_token) = args.priv_token.as_ref() {
        quote_spanned! {priv_token.span() =>
            Protected(#class_name::constructor)
        }
    } else {
        quote_spanned! { class_name.span() =>
            Visible(#class_name::constructor)
        }
    };

    let lua_send_check = {
        let i = args
            .members
            .fields
            .iter()
            .filter(|x| x.r#virtual.is_none())
            .map(|x| {
                let ty = &x.ty;
                quote_spanned! { ty.span() =>
                    bevy_rblx::internal::assert_impl_all!(#ty: bevy_rblx::internal::LuaSend);
                }
            });
        quote! {
            #(#i)*
        }
    };

    quote! {
        pub struct #class_name;

        #members

        #methods

        #new_fn_define

        #lua_send_check

        const _: () = {
            static #vtable_name: bevy_rblx::internal::ObjectVTable = bevy_rblx::internal::ObjectVTable {
                class_name: stringify!(#class_name),
                inherits: &[#(stringify!(#inherits)),*],

                properties: &[#(#property_infos),*],
                methods: &[#(#method_infos),*],

                new: bevy_rblx::internal::ObjectNewFn::#new_fn,

                method_resolution_order: ::std::sync::LazyLock::new(move || bevy_rblx::internal::ObjectVTable::generate_method_resolution_order(stringify!(#class_name))),
                lazy_full_fields: ::std::sync::LazyLock::new(move || bevy_rblx::internal::ObjectVTable::fetch_full_fields(stringify!(#class_name))),
            };

            bevy_rblx::internal::inventory::submit!(bevy_rblx::internal::ObjectVTableCreationPointer(move || &#vtable_name));
        };
    }.into()
}
