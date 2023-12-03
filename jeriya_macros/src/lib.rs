use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{Ident, ImplItem, ImplItemFn, ItemFn, ItemImpl, Type};

#[proc_macro_attribute]
pub fn profile(_args: TokenStream, item: TokenStream) -> TokenStream {
    if let Ok(impl_item_fn) = syn::parse::<ImplItemFn>(item.clone()) {
        return profile_method(impl_item_fn, None);
    }

    if let Ok(item_fn) = syn::parse::<ItemFn>(item.clone()) {
        return profile_fn(item_fn);
    }

    if let Ok(item_impl) = syn::parse::<ItemImpl>(item) {
        return profile_impl(item_impl);
    }

    panic!("profile must be called either on a function or a method");
}

fn profile_method(impl_item_fn: ImplItemFn, ty: Option<&Ident>) -> TokenStream {
    let vis = &impl_item_fn.vis;
    let sig = &impl_item_fn.sig;
    let block = &impl_item_fn.block;

    let fn_name = impl_item_fn.sig.ident.to_string();
    let ident = match ty {
        Some(ty) => format!("{ty}::{fn_name}"),
        None => fn_name,
    };

    let result = quote! {
        #vis #sig {
            let _span = jeriya_shared::span!(#ident);
            #block
        }

    };
    TokenStream::from(result)
}

fn profile_fn(item_fn: ItemFn) -> TokenStream {
    let vis = &item_fn.vis;
    let sig = &item_fn.sig;
    let block = &item_fn.block;

    let ident = item_fn.sig.ident.to_string();
    let result = quote! {
        #vis #sig {
            let _span = jeriya_shared::span!(#ident);
            #block
        }

    };
    TokenStream::from(result)
}

fn profile_impl(item_impl: ItemImpl) -> TokenStream {
    let type_ident = match item_impl.self_ty.as_ref() {
        Type::Path(type_path) => type_path.path.get_ident(),
        _ => None,
    };
    let new_items = item_impl
        .items
        .iter()
        .map(|item| match item {
            ImplItem::Fn(item_fn) => {
                let token_stream = profile_method(item_fn.clone(), type_ident);
                let impl_item_fn: ImplItemFn = syn::parse::<ImplItemFn>(token_stream).unwrap();
                ImplItem::Fn(impl_item_fn)
            }
            item => item.clone(),
        })
        .collect::<Vec<ImplItem>>();
    let new_item_impl = ItemImpl {
        items: new_items,
        ..item_impl
    };
    let result = quote! {
        #new_item_impl
    };
    TokenStream::from(result)
}
