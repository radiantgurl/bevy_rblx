use proc_macro2::TokenStream;
use quote::{self, ToTokens};
use syn::{
    Ident, Lifetime, LitStr, PatType, Result, Token, Type, Visibility, braced, bracketed,
    parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Bracket, Paren},
};

#[derive(Debug, Clone)]
pub(crate) struct CodeBlock {
    pub block: TokenStream,
}

impl Parse for CodeBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        braced!(content in input);

        Ok(Self {
            block: content.parse()?,
        })
    }
}

impl ToTokens for CodeBlock {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let CodeBlock { block } = self;
        tokens.extend(quote::quote! {
            {
                #block
            }
        });
    }
}

#[derive(Clone)]
pub(crate) enum AttrArgValue {
    Expr(syn::Expr),
    Func(syn::Signature),
}

#[derive(Clone)]
pub(crate) struct AttrNamedArg {
    pub name: Ident,
    pub _assign_token: Token![=],
    pub value: AttrArgValue,
}
#[derive(Clone)]
pub(crate) enum AttrArg {
    Named(AttrNamedArg),
}

#[derive(Clone)]
pub(crate) struct AttrArguments {
    pub args: Punctuated<AttrArg, Token![,]>,
}

impl Parse for AttrArgValue {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(syn::Token![fn]) {
            Ok(AttrArgValue::Func(input.parse()?))
        } else {
            Ok(AttrArgValue::Expr(input.parse()?))
        }
    }
}

impl Parse for AttrNamedArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;
        let assign_token = input.parse()?;
        let value = input.parse()?;
        Ok(AttrNamedArg {
            name,
            _assign_token: assign_token,
            value,
        })
    }
}

impl Parse for AttrArg {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek2(Token![=]) {
            Ok(AttrArg::Named(input.parse()?))
        } else {
            unimplemented!()
        }
    }
}

impl Parse for AttrArguments {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.is_empty() {
            return Ok(AttrArguments {
                args: Punctuated::new(),
            });
        }
        Ok(AttrArguments {
            args: input.parse_terminated(AttrArg::parse, Token![,])?,
        })
    }
}

impl ToTokens for AttrArgValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            AttrArgValue::Expr(expr) => expr.to_tokens(tokens),
            AttrArgValue::Func(sig) => sig.to_tokens(tokens),
        }
    }
}

impl TryInto<syn::Expr> for AttrArgValue {
    type Error = syn::Error;

    fn try_into(self) -> Result<syn::Expr> {
        match self {
            AttrArgValue::Expr(expr) => Ok(expr),
            x => Err(syn::Error::new(x.span(), "expected expression")),
        }
    }
}

impl TryInto<syn::Signature> for AttrArgValue {
    type Error = syn::Error;

    fn try_into(self) -> Result<syn::Signature> {
        match self {
            AttrArgValue::Func(sig) => Ok(sig),
            x => Err(syn::Error::new(x.span(), "expected function")),
        }
    }
}

impl TryInto<syn::Path> for AttrArgValue {
    type Error = syn::Error;

    fn try_into(self) -> Result<syn::Path> {
        let span = self.span();
        if let Self::Expr(syn::Expr::Path(p)) = self {
            Ok(p.path)
        } else {
            Err(syn::Error::new(span, "expected path"))
        }
    }
}

impl TryInto<syn::Ident> for AttrArgValue {
    type Error = syn::Error;

    fn try_into(self) -> Result<syn::Ident> {
        let span = self.span();
        if let Self::Expr(syn::Expr::Path(p)) = self {
            if p.path.segments.len() == 1 {
                Ok(p.path.segments[0].ident.clone())
            } else {
                Err(syn::Error::new(
                    p.path.span(),
                    "expected identifier, got path",
                ))
            }
        } else {
            Err(syn::Error::new(span, "expected identifier"))
        }
    }
}

impl AttrArguments {
    pub fn get_named_arg(&self, name: &str) -> Option<&AttrNamedArg> {
        self.args.iter().find_map(|arg| match arg {
            AttrArg::Named(named) => {
                if named.name == name {
                    Some(named)
                } else {
                    None
                }
            }
        })
    }
}

mod kw {
    use syn::{custom_keyword, parse::Parse};

    custom_keyword!(members);
    custom_keyword!(methods);
    custom_keyword!(security);
    custom_keyword!(rename);
    custom_keyword!(read_only);
    custom_keyword!(getter);
    custom_keyword!(setter);
    custom_keyword!(default);
    custom_keyword!(lua);
    custom_keyword!(Lua);
    custom_keyword!(Entity);
    custom_keyword!(ObjectRef);
    custom_keyword!(LuaResult);
    custom_keyword!(LuaValue);
    custom_keyword!(ObjectVTable);
    custom_keyword!(bool);
    custom_keyword!(deprecated_alias);
    custom_keyword!(require_components);
    custom_keyword!(EntityCommands);
    custom_keyword!(custom_new);
    custom_keyword!(post_init);

    #[derive(Clone, Copy)]
    pub(crate) struct End;

    impl Parse for End {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            if input.is_empty() {
                Ok(End)
            } else {
                Err(input.error("expected end"))
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct MethodMeta {
    pub rename: Option<LitStr>,
    pub security: Option<Ident>,
    pub deprecated_aliases: Vec<LitStr>,

    // keywords by themselves
    pub r#override: Option<Token![override]>,
}

impl Default for MethodMeta {
    fn default() -> Self {
        Self {
            rename: Default::default(),
            security: Default::default(),
            deprecated_aliases: Default::default(),

            r#override: Default::default(),
        }
    }
}

impl Parse for MethodMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut meta = MethodMeta::default();
        while input.peek(Token![#]) {
            input.parse::<Token![#]>()?;
            let content;
            let _: Bracket = bracketed!(content in input);
            if content.peek(kw::rename) {
                content.parse::<kw::rename>()?;
                content.parse::<Token![=]>()?;
                meta.rename = Some(content.parse::<LitStr>()?);
                content.parse::<kw::End>()?;
            } else if content.peek(kw::deprecated_alias) {
                content.parse::<kw::deprecated_alias>()?;
                content.parse::<Token![=]>()?;
                meta.deprecated_aliases.push(content.parse()?);
                content.parse::<kw::End>()?;
            } else {
                content.parse::<kw::security>()?;
                content.parse::<Token![=]>()?;
                meta.security = Some(content.parse::<Ident>()?);
                content.parse::<kw::End>()?;
            }
        }
        while !input.peek(Token![fn]) && !input.peek(Token![async]) {
            if input.peek(Token![override]) && meta.r#override.is_none() {
                meta.r#override = Some(input.parse()?);
            } else {
                Err(input.error(match &meta.r#override {
                    None => "expected fn, async or override",
                    Some(_) => "expected fn or async",
                }))?;
            }
        }
        Ok(meta)
    }
}

pub(crate) struct LuaMethod<Args, Return> {
    pub async_token: Option<Token![async]>,
    pub name: Ident,
    pub lua_arg: TokenStream,
    pub self_name: Ident,
    pub self_ty: Ident,
    pub args: Args,
    pub lua_result: kw::LuaResult,
    pub lt: Token![<],
    pub return_type: Return,
    pub gt: Token![>],
}

pub(crate) struct LuaMethodClosure<Args, Return> {
    pub async_token: Option<Token![async]>,
    pub lua_arg: TokenStream,
    pub self_name: Ident,
    pub self_ty: Ident,
    pub args: Args,
    pub lua_result: kw::LuaResult,
    pub lt: Token![<],
    pub return_type: Return,
    pub gt: Token![>],
}

impl<Args: Clone, Return: Clone> Clone for LuaMethod<Args, Return> {
    fn clone(&self) -> Self {
        Self {
            async_token: self.async_token.clone(),
            name: self.name.clone(),
            lua_arg: self.lua_arg.clone(),
            self_name: self.self_name.clone(),
            self_ty: self.self_ty.clone(),
            args: self.args.clone(),
            return_type: self.return_type.clone(),
            lua_result: self.lua_result.clone(),
            lt: self.lt.clone(),
            gt: self.gt.clone(),
        }
    }
}

impl<Args: Clone, Return: Clone> Clone for LuaMethodClosure<Args, Return> {
    fn clone(&self) -> Self {
        Self {
            async_token: self.async_token.clone(),
            lua_arg: self.lua_arg.clone(),
            self_name: self.self_name.clone(),
            self_ty: self.self_ty.clone(),
            args: self.args.clone(),
            return_type: self.return_type.clone(),
            lua_result: self.lua_result.clone(),
            lt: self.lt.clone(),
            gt: self.gt.clone(),
        }
    }
}

impl<Args: Parse, Return: Parse> Parse for LuaMethod<Args, Return> {
    fn parse(input: ParseStream) -> Result<Self> {
        let async_token: Option<Token![async]> = input.parse()?;
        input.parse::<Token![fn]>()?;
        let name = input.parse()?;
        let self_name;
        let self_ty;
        let args;
        let lua_arg;
        {
            let args_buffer;
            parenthesized!(args_buffer in input);
            let lua_kw = args_buffer.parse::<kw::lua>()?;
            args_buffer.parse::<Token![:]>()?;
            if async_token.is_none() {
                args_buffer.parse::<Token![&]>()?;
            }
            let lua_ty = args_buffer.parse::<kw::Lua>()?;
            lua_arg = if async_token.is_some() {
                quote::quote! {
                    #lua_kw: #lua_ty
                }
            } else {
                quote::quote! {
                    #lua_kw: &#lua_ty
                }
            };
            args_buffer.parse::<Token![,]>()?;
            self_name = args_buffer.parse::<Ident>()?;
            args_buffer.parse::<Token![:]>()?;
            if args_buffer.peek(kw::ObjectRef) || args_buffer.peek(kw::Entity) {
                self_ty = args_buffer.parse::<Ident>()?;
            } else {
                Err(args_buffer.error("expected Entity or ObjectRef"))?;
                unreachable!();
            }
            args = args_buffer.parse()?;
            args_buffer.parse::<kw::End>()?;
        }

        input.parse::<Token![->]>()?;
        let lua_result = input.parse::<kw::LuaResult>()?;
        let lt = input.parse::<Token![<]>()?;
        let return_type = input.parse()?;
        let gt = input.parse::<Token![>]>()?;
        Ok(Self {
            async_token,
            name,
            lua_arg,
            self_name,
            self_ty,
            args,
            return_type,
            lua_result,
            lt,
            gt,
        })
    }
}

impl<Args: Parse, Return: Parse> Parse for LuaMethodClosure<Args, Return> {
    fn parse(input: ParseStream) -> Result<Self> {
        let async_token: Option<Token![async]> = input.parse()?;
        input.parse::<Token![fn]>()?;
        let self_name;
        let self_ty;
        let lua_arg;
        let args;
        {
            let args_buffer;
            parenthesized!(args_buffer in input);
            let lua_kw = args_buffer.parse::<kw::lua>()?;
            args_buffer.parse::<Token![:]>()?;
            if async_token.is_none() {
                args_buffer.parse::<Token![&]>()?;
            }
            let lua_ty = args_buffer.parse::<kw::Lua>()?;
            lua_arg = if async_token.is_some() {
                quote::quote! {
                    #lua_kw: #lua_ty
                }
            } else {
                quote::quote! {
                    #lua_kw: &#lua_ty
                }
            };
            args_buffer.parse::<Token![,]>()?;
            self_name = args_buffer.parse::<Ident>()?;
            args_buffer.parse::<Token![:]>()?;
            if args_buffer.peek(kw::ObjectRef) || args_buffer.peek(kw::Entity) {
                self_ty = args_buffer.parse::<Ident>()?;
            } else {
                Err(args_buffer.error("expected Entity or ObjectRef"))?;
                unreachable!();
            }
            args = args_buffer.parse()?;
            args_buffer.parse::<kw::End>()?;
        }

        input.parse::<Token![->]>()?;
        let lua_result = input.parse::<kw::LuaResult>()?;
        let lt = input.parse::<Token![<]>()?;
        let return_type = input.parse()?;
        let gt = input.parse::<Token![>]>()?;
        Ok(Self {
            async_token,
            lua_arg,
            self_name,
            self_ty,
            args,
            return_type,
            lua_result,
            lt,
            gt,
        })
    }
}

#[derive(Default, Clone)]
pub(crate) struct GenericLuaArgs {
    pub args: Vec<PatType>,
    pub variadic: Option<Ident>,
}

impl Parse for GenericLuaArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut args = Self::default();
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            while !input.peek(syn::parse::End) && !input.peek3(Token![...]) {
                args.args.push(input.parse()?);
                if !input.peek(syn::parse::End) {
                    input.parse::<Token![,]>()?;
                }
            }
            if input.peek3(Token![...]) {
                args.variadic = Some(input.parse()?);
                input.parse::<Token![:]>()?;
                input.parse::<Token![...]>()?;
            }
        }
        Ok(args)
    }
}

impl ToTokens for GenericLuaArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let GenericLuaArgs { args, variadic } = self;
        let pat = args.iter().map(|x| &x.pat);
        let ty = args.iter().map(|x| &x.ty);
        if let Some(v) = variadic {
            tokens.extend(quote::quote! {
                , (#(#pat),* , #v) : (#(#ty),* , bevy_rblx::internal::LuaMultiValue)
            });
        } else {
            tokens.extend(quote::quote! {
                , (#(#pat),*) : (#(#ty),*)
            });
        }
    }
}

impl GenericLuaArgs {
    pub fn add_self_type(&self, self_name: &Ident, self_ty: &Ident) -> TokenStream {
        let mut tokens = TokenStream::new();
        let GenericLuaArgs { args, variadic } = self;
        let pat = args.iter().map(|x| &x.pat);
        let ty = args.iter().map(|x| &x.ty);
        if let Some(v) = variadic {
            tokens.extend(quote::quote! {
                , (#self_name,#(#pat),* , #v) : (#self_ty,#(#ty),* , bevy_rblx::internal::LuaMultiValue)
            });
        } else {
            tokens.extend(quote::quote! {
                , (#self_name,#(#pat),*) : (#self_ty,#(#ty),*)
            });
        }
        tokens
    }
}

pub(crate) type GenericLuaMethod = LuaMethod<GenericLuaArgs, syn::Type>;

#[derive(Clone)]
pub(crate) struct GetterLuaArgs {
    pub comma: syn::token::Comma,
    pub vtable_ident: Ident,
    pub ampersand: syn::token::And,
    pub static_lifetime: syn::Lifetime,
    pub ty: kw::ObjectVTable,
}
impl Parse for GetterLuaArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let comma = input.parse::<Token![,]>()?;
        let vtable_ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ampersand = input.parse::<Token![&]>()?;
        if let Some((l, _)) = input.cursor().lifetime() {
            if l.ident != "static" {
                return Err(input.error("expected 'static"));
            }
        } else {
            return Err(input.error("expected 'static"));
        }
        let static_lifetime = input.parse::<Lifetime>()?;
        let ty = input.parse::<kw::ObjectVTable>()?;
        Ok(Self {
            vtable_ident,
            comma,
            ampersand,
            static_lifetime,
            ty,
        })
    }
}
impl ToTokens for GetterLuaArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let GetterLuaArgs {
            comma,
            vtable_ident,
            ampersand,
            static_lifetime,
            ty,
        } = self;
        tokens.extend(quote::quote! {
            #comma #vtable_ident: #ampersand #static_lifetime bevy_rblx::internal::#ty
        });
    }
}
#[derive(Clone)]
pub(crate) struct SetterLuaArgs {
    pub comma: syn::token::Comma,
    pub vtable_ident: Ident,
    pub ampersand: syn::token::And,
    pub static_lifetime: syn::Lifetime,
    pub ty: kw::ObjectVTable,
    pub new_comma: syn::token::Comma,
    pub new_value_ident: Ident,
    pub lua_value_ty: kw::LuaValue,
}
impl Parse for SetterLuaArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let comma = input.parse::<Token![,]>()?;
        let vtable_ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ampersand = input.parse::<Token![&]>()?;
        if let Some((l, _)) = input.cursor().lifetime() {
            if l.ident != "static" {
                return Err(input.error("expected 'static"));
            }
        } else {
            return Err(input.error("expected 'static"));
        }
        let static_lifetime = input.parse::<Lifetime>()?;
        let ty = input.parse::<kw::ObjectVTable>()?;
        let new_comma = input.parse::<Token![,]>()?;
        let new_value_ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let lua_value_ty = input.parse::<kw::LuaValue>()?;
        Ok(SetterLuaArgs {
            vtable_ident,
            new_value_ident,
            comma,
            ampersand,
            static_lifetime,
            ty,
            new_comma,
            lua_value_ty,
        })
    }
}

impl ToTokens for SetterLuaArgs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let SetterLuaArgs {
            comma,
            vtable_ident,
            ampersand,
            static_lifetime,
            ty,
            new_comma,
            new_value_ident,
            lua_value_ty,
        } = self;
        tokens.extend(quote::quote! {
            #comma #vtable_ident: #ampersand #static_lifetime bevy_rblx::internal::#ty #new_comma #new_value_ident: #lua_value_ty
        })
    }
}
pub(crate) type GetterLuaMethod = LuaMethodClosure<GetterLuaArgs, kw::LuaValue>;
pub(crate) type SetterLuaMethod = LuaMethodClosure<SetterLuaArgs, kw::bool>;
pub(crate) type PostInitFn = LuaMethodClosure<kw::End, UnitType>;

#[derive(Clone)]
pub(crate) struct Method<MethodTy: Parse + Clone> {
    pub meta: MethodMeta,
    pub sign: MethodTy,
    pub code: CodeBlock,
}

impl<MethodTy: Parse + Clone> Parse for Method<MethodTy> {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            meta: input.parse()?,
            sign: input.parse()?,
            code: input.parse()?,
        })
    }
}

#[derive(Clone)]
pub(crate) struct UnitType(Paren);
impl ToTokens for UnitType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(quote::quote_spanned! { self.0.span.span() => () })
    }
}
impl Parse for UnitType {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let unit = Self {
            0: parenthesized!(content in input),
        };
        content.parse::<kw::End>()?;
        Ok(unit)
    }
}

#[derive(Clone)]
pub(crate) struct MethodList(pub Vec<Method<GenericLuaMethod>>);

impl Parse for MethodList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut v = Vec::new();
        while !input.is_empty() {
            v.push(input.parse()?);
        }
        Ok(Self(v))
    }
}

#[derive(Clone)]
pub(crate) struct NewFn {
    pub lua_arg: TokenStream,
    pub entity_command_ident: Ident,
    pub entity_command_type: kw::EntityCommands,
    pub lua_result: kw::LuaResult,
    pub lt: Token![<],
    pub return_type: UnitType,
    pub gt: Token![>],
    pub code: CodeBlock,
}

impl Parse for NewFn {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![fn]>()?;
        let lua_arg;
        let entity_command_ident;
        let entity_command_type;
        {
            let args_buffer;
            parenthesized!(args_buffer in input);

            let lua_name = args_buffer.parse::<kw::lua>()?;
            args_buffer.parse::<Token![:]>()?;
            let and = args_buffer.parse::<Token![&]>()?;
            let lua_type = args_buffer.parse::<kw::Lua>()?;
            lua_arg = quote::quote! {
                #lua_name: #and #lua_type
            };
            args_buffer.parse::<Token![,]>()?;
            args_buffer.parse::<Token![mut]>()?;
            entity_command_ident = args_buffer.parse::<Ident>()?;
            args_buffer.parse::<Token![:]>()?;
            entity_command_type = args_buffer.parse::<kw::EntityCommands>()?;
            args_buffer.parse::<kw::End>()?;
        }
        input.parse::<Token![->]>()?;
        Ok(Self {
            lua_arg,
            entity_command_ident,
            entity_command_type,
            lua_result: input.parse()?,
            lt: input.parse()?,
            return_type: input.parse()?,
            gt: input.parse()?,
            code: input.parse()?,
        })
    }
}

#[derive(Clone)]
pub(crate) struct ObjectField {
    pub getter: Option<(GetterLuaMethod, CodeBlock)>,
    pub setter: Option<(SetterLuaMethod, CodeBlock)>,
    pub rename: Option<LitStr>,
    pub security: Option<Ident>,
    pub default: Option<syn::Expr>,
    pub read_only: Option<kw::read_only>,
    pub deprecated_aliases: Vec<LitStr>,

    pub visibility: Visibility,

    pub r#priv: Option<Token![priv]>,
    pub r#virtual: Option<Token![virtual]>,
    pub name: Ident,
    pub ty: Type,
}

impl Parse for ObjectField {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut getter = None;
        let mut setter = None;
        let mut rename = None;
        let mut security = None;
        let mut default = None;
        let mut read_only = None;
        let mut deprecated_aliases = Vec::new();

        while input.peek(Token![#]) {
            input.parse::<Token![#]>()?;
            let content;
            let _: Bracket = bracketed!(content in input);
            if content.peek(kw::rename) {
                content.parse::<kw::rename>()?;
                content.parse::<Token![=]>()?;
                rename = Some(content.parse::<LitStr>()?);
                content.parse::<kw::End>()?;
            } else if content.peek(kw::default) {
                content.parse::<kw::default>()?;
                content.parse::<Token![=]>()?;
                default = Some(content.parse::<syn::Expr>()?);
                content.parse::<kw::End>()?;
            } else if content.peek(kw::getter) {
                content.parse::<kw::getter>()?;
                content.parse::<Token![=]>()?;
                getter = Some((
                    content.parse::<GetterLuaMethod>()?,
                    content.parse::<CodeBlock>()?,
                ));
                content.parse::<kw::End>()?;
            } else if content.peek(kw::setter) {
                content.parse::<kw::setter>()?;
                content.parse::<Token![=]>()?;
                setter = Some((
                    content.parse::<SetterLuaMethod>()?,
                    content.parse::<CodeBlock>()?,
                ));
                content.parse::<kw::End>()?;
            } else if content.peek(kw::deprecated_alias) {
                content.parse::<kw::deprecated_alias>()?;
                content.parse::<Token![=]>()?;
                deprecated_aliases.push(content.parse()?);
                content.parse::<kw::End>()?;
            } else if content.peek(kw::read_only) {
                read_only = Some(content.parse::<kw::read_only>()?);
                content.parse::<kw::End>()?;
            } else {
                content.parse::<kw::security>()?;
                content.parse::<Token![=]>()?;
                security = Some(content.parse::<Ident>()?);
                content.parse::<kw::End>()?;
            }
        }
        let visibility = input.parse()?;
        let r#priv = input.parse()?;
        if input.peek(Token![virtual]) && getter.is_none() {
            Err(input.error("expected getter to be defined to allow virtual field"))?;
        }
        let r#virtual = input.parse()?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        Ok(Self {
            getter,
            setter,
            rename,
            security,
            default,
            r#priv,
            r#virtual,
            name,
            ty,
            read_only,
            visibility,
            deprecated_aliases,
        })
    }
}

#[derive(Clone)]
pub(crate) struct FieldList {
    _braces: Brace,
    pub fields: Punctuated<ObjectField, Token![,]>,
}

impl Parse for FieldList {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        Ok(Self {
            _braces: braced!(content in input),
            fields: Punctuated::parse_terminated(&content)?,
        })
    }
}

pub(crate) struct ClassArgs {
    pub require_components: Option<Punctuated<Type, Token![,]>>,
    pub custom_constructor: Option<NewFn>,
    pub post_init: Option<(PostInitFn, CodeBlock)>,
    pub priv_token: Option<Token![priv]>,
    pub abstract_token: Option<Token![abstract]>,
    pub class_name: Ident,
    _inherits_paren: Paren,
    pub inherits: Punctuated<Ident, Token![,]>,
    pub members_token: kw::members,
    pub members: FieldList,
    pub methods_token: kw::methods,
    _methods_brace_token: Brace,
    pub methods: MethodList,
}

impl Parse for ClassArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut require_components = None;
        let mut custom_constructor = None;
        let mut post_init = None;
        while input.peek(Token![#]) {
            input.parse::<Token![#]>()?;
            let content;
            bracketed!(content in input);
            if content.peek(kw::require_components) {
                let paren_content;
                content.parse::<kw::require_components>()?;
                parenthesized!(paren_content in content);
                require_components = Some(Punctuated::parse_terminated(&paren_content)?);
                content.parse::<kw::End>()?;
            } else if content.peek(kw::post_init) {
                content.parse::<kw::post_init>()?;
                content.parse::<Token![=]>()?;
                post_init = Some((content.parse::<PostInitFn>()?, content.parse()?));
            } else {
                content.parse::<kw::custom_new>()?;
                content.parse::<Token![=]>()?;
                custom_constructor = Some(content.parse::<NewFn>()?);
            }
        }
        // let no_instance = if input.peek(Token![#]) {
        //     Some(input.parse()?)
        // } else {
        //     None
        // };
        let inherits_content;
        let members_content;
        Ok(Self {
            priv_token: input.parse()?,
            abstract_token: input.parse()?,
            class_name: input.parse()?,
            _inherits_paren: parenthesized!(inherits_content in input),
            inherits: Punctuated::<_, _>::parse_terminated(&inherits_content)?,
            members_token: input.parse()?,
            members: input.parse()?,
            methods_token: input.parse()?,
            _methods_brace_token: braced!(members_content in input),
            methods: members_content.parse()?,
            require_components,
            custom_constructor,
            post_init,
        })
    }
}
