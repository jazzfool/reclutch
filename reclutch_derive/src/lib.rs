extern crate proc_macro;

use {crate::proc_macro::TokenStream, quote::quote};

#[proc_macro_derive(Widget, attributes(widget_child))]
pub fn widget_macro_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_widget_macro(&ast)
}

fn impl_widget_macro(ast: &syn::DeriveInput) -> TokenStream {
    enum StringOrInt {
        String(String),
        Int(usize),
    }

    let name = &ast.ident;

    let mut children = Vec::new();

    match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => {
                for field in fields.named.iter() {
                    if let Some(ref ident) = field.ident {
                        let mut is_child = false;
                        for attr in &field.attrs {
                            if attr.path.segments.len() > 0 {
                                if attr.path.segments[0].ident == "widget_child" {
                                    is_child = true;
                                    break;
                                }
                            }
                        }

                        if is_child {
                            children.push(StringOrInt::String(ident.to_string()));
                        }
                    }
                }
            }
            syn::Fields::Unnamed(fields) => {
                for (i, field) in fields.unnamed.iter().enumerate() {
                    let mut is_child = false;
                    for attr in &field.attrs {
                        if attr.path.segments.len() > 0 {
                            if attr.path.segments[0].ident == "widget_child" {
                                is_child = true;
                                break;
                            }
                        }
                    }

                    if is_child {
                        children.push(StringOrInt::Int(i));
                    }
                }
            }
            _ => (),
        },
        _ => (),
    };

    let mut_children: Vec<_> = children.iter().map(|child| {
        match child {
            StringOrInt::String(child) => {
                let ident = quote::format_ident!("{}", child);
                quote! {
                    &mut self.#ident
                }
            }
            StringOrInt::Int(child) => {
                let ident = syn::Index::from(*child );
                quote! {
                    &mut self.#ident
                }
            }
        }
    }).collect();
    let children: Vec<_> = children.iter().map(|child| {
        match child {
            StringOrInt::String(child) => {
                let ident = quote::format_ident!("{}", child);
                quote! {
                    &self.#ident
                }
            }
            StringOrInt::Int(child) => {
                let ident = syn::Index::from(*child);
                quote! {
                    &self.#ident
                }
            }
        }
    }).collect();

    let tokens = quote! {
        impl reclutch::WidgetChildren for #name {
            fn children(&self) -> Vec<&dyn Widget> {
                vec![ #(#children),* ]
            }

            fn children_mut(&mut self) -> Vec<&mut dyn Widget> {
                vec![ #(#mut_children),* ]
            }
        }
    };

    tokens.into()
}
