use darling::FromMeta;
use proc_macro2::Ident;
use quote::ToTokens;
use regex::Regex;
use syn::{parse2, parse_macro_input};

#[proc_macro_attribute]
pub fn rpc(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr = proc_macro2::TokenStream::from(attr);
    let item = proc_macro2::TokenStream::from(item);

    rpc_inner(attr, item).into()
}

fn rpc_inner(
    attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let mut t: syn::ItemTrait = parse2(item).unwrap();

    let functions = t.items.iter_mut().filter_map(|item| match item {
        syn::TraitItem::Fn(method) => Some(method),
        _ => None,
    });

    let mut new_impls: Vec<_> = vec![];
    for function in functions {
        let pos = function
            .attrs
            .iter()
            .position(|a| a.path().is_ident("rpc"))
            .unwrap();

        let attr = function.attrs.remove(pos);

        let attr = attr.meta.require_list().unwrap();

        let attr_args = match darling::export::NestedMeta::parse_meta_list(attr.tokens.clone()) {
            Ok(v) => v,
            Err(e) => {
                return darling::Error::from(e).write_errors();
            }
        };

        let parsed_attrs = FnAttributeArgs::from_list(&attr_args).unwrap();

        let route = parsed_attrs.route;
        let method = parsed_attrs.method;
        let mut function_copy = function.clone();

        let fun_sig_attrs = function_copy
            .sig
            .inputs
            .iter()
            .filter_map(|f| match f {
                syn::FnArg::Typed(t) => Some(t),
                _ => None,
            })
            .collect::<Vec<_>>();

        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"\{([[^\[\]]&&[[:word:]]]*)\}").unwrap();
        }

        let route_attrs = RE
            .captures_iter(&route)
            .map(|x| x.get(1).unwrap().as_str())
            .collect::<Vec<_>>();

        let body_attribute = fun_sig_attrs
            .iter()
            .find(|x| !route_attrs.contains(&x.pat.to_token_stream().to_string().as_str()));

        let body_block = if let Some(body_attr) = body_attribute {
            let pat = &body_attr.pat;

            quote::quote! {
                let body = serde_json::to_value(#pat).map_err(ClientError::Serialization)?;
                let body = Some(body);
            }
        } else {
            quote::quote! { let body = None; }
        };

        let route_hash_inserts = route_attrs.iter().map(|x| {
            let ident = Ident::new(x, proc_macro2::Span::call_site());

            quote::quote! { vars.insert(#x.to_string(), #ident); }
        });

        function_copy.default = Some(
            parse2(quote::quote! { {
                let method = #method;
                let route = #route;

                let mut vars = std::collections::HashMap::<String, String>::new();
                #(#route_hash_inserts)*
                let vars = vars;
                let route = strfmt::strfmt(&route, &vars).unwrap();

                #body_block
                let resp = self
                    .send_request(method, &route, body)
                    .await
                    .map_err(ClientError::Transport)?;

                match resp {
                    Ok(resp) => {
                        let resp = serde_json::from_value(resp).map_err(ClientError::Deserialization)?;
                        Ok(resp)
                    }
                    Err(resp) => {
                        let resp = serde_json::from_value(resp).map_err(ClientError::Deserialization)?;
                        Err(ClientError::Api(resp))
                    }
                }
     } })
            .unwrap(),
        );
        new_impls.push(function_copy);
        // let name = &function.sig.ident;
        // let args = &function.sig.inputs;
        // let ret = &function.sig.output;

        // let upcased = name.to_string().to_uppercase();
        // let const_route_name = format!("{upcased}_ROUTE");

        // println!("name: \"{}\"", name.to_string());
        // println!("args: \"{:?}\"", args);
        // println!("ret: \"{:?}\"", ret);
    }

    // dbg!(&new_impls);
    let client_transport_impl = quote::quote! {
        impl<Transport> Api for Transport
        where
            Transport: ClientTransport,
        {
            type ErrorWrapper<InnerError: Debug + for<'a> Deserialize<'a>> =
                ClientError<InnerError, Transport::Error>;

            #(#new_impls)*
        }
    };

    quote::quote! {
        #t

        #client_transport_impl
    }
}

#[derive(Debug, darling::FromMeta)]
struct FnAttributeArgs {
    method: Option<String>,
    route: String,
}
