use proc_macro2::{Ident, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    parse_macro_input, punctuated::Punctuated, spanned::Spanned, token::Comma, Data,
    DeriveInput, Expr, LitInt, Meta, Path, Variant,
};

#[proc_macro_derive(ErrorStatus, attributes(status))]
pub fn derive_error_status(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = parse_macro_input!(input);
    let enum_ident = ast.ident;
    let cases: Punctuated<TokenStream, Comma> = match ast.data {
        Data::Enum(data) => data.variants,
        _ => panic!(
            "#[derive(ErrorStatus)] is only available for enums, other types are not supported."
        ),
    }
    .iter()
    .map(impl_enum_variant)
    .collect();

    quote! {
        impl axum::response::IntoResponse for #enum_ident {
            fn into_response(self) -> axum::response::Response {
                tracing::error!("Error status: {:?}", self);
                match self {
                    #cases
                }
            }
        }
    }
    .into()
}

fn impl_enum_variant(input: &Variant) -> TokenStream {
    let status_code = find_status_code(input);
    let case = if input.fields.is_empty() {
        case_empty_fields(input)
    } else if input.fields.iter().filter(|f| f.ident.is_none()).count() > 0 {
        case_unnamed_fields(input)
    } else {
        case_named_fields(input)
    };
    quote! {
        Self::#case => (#status_code, format!("{}", self)).into_response()
    }
}

fn case_empty_fields(input: &Variant) -> TokenStream {
    let ident = &input.ident;
    quote!(#ident)
}

fn case_unnamed_fields(input: &Variant) -> TokenStream {
    let ident = &input.ident;
    quote!(#ident( .. ))
}

fn case_named_fields(input: &Variant) -> TokenStream {
    let ident = &input.ident;
    quote!(#ident { .. })
}

fn find_status_code(input: &Variant) -> TokenStream {
    match input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("status"))
    {
        Some(attr) => match &attr.meta {
            Meta::List(l) => {
                if let Ok(number) = l.parse_args::<LitInt>() {
                    quote! {
                        axum::http::StatusCode::from_u16(#number as u16).unwrap()
                    }
                } else if let Ok(expr) = l.parse_args::<Path>() {
                    quote! {
                        #expr
                    }
                } else {
                    quote_spanned!(l.span() => compile_error!("Only #[status(StatusCode)] or #[status(u16)] syntaxe is supported"))
                }
            }
            _ => {
                quote_spanned! { attr.span() => compile_error!("Only #[status(StatusCode)] or #[status(u16)] syntaxe is supported") }
            }
        },
        None => {
            quote_spanned! { input.span() => compile_error!("Each enum variant should have a status code provided using the #[status()] attribute") }
        }
    }
}
