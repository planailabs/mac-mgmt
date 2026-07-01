//! `#[api_mcp_dioxus_server]` — generate the thin Dioxus `#[server]` wrapper for
//! a `plan-ai-api-mcp` endpoint handler, so domain logic is written once.
//!
//! Applied to an endpoint handler of the shape
//! `async fn NAME(pool: &Pool, p: &Principal, input: In) -> Result<Out, ApiError>`,
//! it emits:
//!   * the original handler, gated behind `#[cfg(feature = "server")]` (it uses
//!     server-only types like the DB pool), and
//!   * an ungated Dioxus `#[server]` function (client + server) that resolves the
//!     principal from the web session and delegates to the handler.
//!
//! ```ignore
//! #[api_mcp_dioxus_server(server = "list_domains")]
//! pub async fn domain_list(pool: &PgPool, p: &Principal, input: DomainListInput)
//!     -> Result<Vec<DomainRow>, ApiError> { /* ... */ }
//!
//! // generated:
//! // #[server]
//! // pub async fn list_domains(input: DomainListInput)
//! //     -> Result<Vec<DomainRow>, ServerFnError> {
//! //     let __principal = principal_from(&current_user().await?);
//! //     domain_list(&server_pool()?, &__principal, input).await.map_err(to_serverfn)
//! // }
//! ```
//!
//! The generated wrapper calls four items that must be in scope where the macro
//! is used (bring them in with one `use` per endpoints module):
//! `current_user`, `principal_from`, `server_pool`, `to_serverfn`.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    Error, FnArg, GenericArgument, Ident, ItemFn, LitStr, PathArguments, ReturnType, Token, Type,
    parse_macro_input,
};

/// Parsed attribute arguments: `server = "fn_name"`, optional `cfg = "feature"`.
struct Args {
    server: Ident,
    cfg_feature: String,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut server: Option<Ident> = None;
        let mut cfg_feature = String::from("server");
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let val: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "server" => server = Some(Ident::new(&val.value(), val.span())),
                "cfg" => cfg_feature = val.value(),
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!("unknown argument `{other}` (expected `server` or `cfg`)"),
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        let server = server.ok_or_else(|| {
            Error::new(
                proc_macro2::Span::call_site(),
                "missing required `server = \"fn_name\"` argument",
            )
        })?;
        Ok(Args {
            server,
            cfg_feature,
        })
    }
}

#[proc_macro_attribute]
pub fn api_mcp_dioxus_server(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as Args);
    let func = parse_macro_input!(item as ItemFn);

    // The 3rd typed parameter is the endpoint input.
    let input_ty = match func.sig.inputs.iter().nth(2) {
        Some(FnArg::Typed(pt)) => (*pt.ty).clone(),
        _ => {
            return Error::new_spanned(
                &func.sig,
                "expected an endpoint of the form `async fn f(pool, principal, input) -> Result<Out, ApiError>`",
            )
            .to_compile_error()
            .into();
        }
    };

    // The Ok type of the `Result<Ok, ApiError>` return.
    let ok_ty = match result_ok_type(&func.sig.output) {
        Ok(ty) => ty,
        Err(e) => return e.to_compile_error().into(),
    };

    let fn_name = &func.sig.ident;
    let server_name = &args.server;
    let cfg_feature = &args.cfg_feature;

    let expanded = quote! {
        #[cfg(feature = #cfg_feature)]
        #func

        #[server]
        pub async fn #server_name(input: #input_ty) -> Result<#ok_ty, ServerFnError> {
            let __principal = principal_from(&current_user().await?);
            #fn_name(&server_pool()?, &__principal, input).await.map_err(to_serverfn)
        }
    };
    expanded.into()
}

/// Extract `Ok` from a `-> Result<Ok, E>` return type.
fn result_ok_type(output: &ReturnType) -> syn::Result<Type> {
    let ty = match output {
        ReturnType::Type(_, ty) => ty.as_ref(),
        ReturnType::Default => {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "endpoint must return `Result<Out, ApiError>`",
            ));
        }
    };
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Result" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(ok)) = args.args.first() {
                        return Ok(ok.clone());
                    }
                }
            }
        }
    }
    Err(Error::new_spanned(
        ty,
        "endpoint must return `Result<Out, ApiError>`",
    ))
}
