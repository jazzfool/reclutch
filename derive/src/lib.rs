extern crate proc_macro;

mod event;

use {proc_macro::TokenStream, quote::quote};

#[proc_macro_derive(
    WidgetChildren,
    attributes(widget_child, vec_widget_child, widget_children_trait)
)]
pub fn widget_macro_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_widget_macro(&ast)
}

enum ChildAttr {
    None,
    WidgetChild,
    VecWidgetChild,
}

enum StringOrInt {
    String(String),
    Int(usize),
}

enum ChildReference {
    Single(StringOrInt),
    Vec(StringOrInt),
}

fn chk_attrs_is_child(attrs: &[syn::Attribute]) -> ChildAttr {
    for attr in attrs {
        if attr.path.segments.first().map(|i| i.ident == "widget_child").unwrap_or(false) {
            return ChildAttr::WidgetChild;
        } else if attr.path.segments.first().map(|i| i.ident == "vec_widget_child").unwrap_or(false)
        {
            return ChildAttr::VecWidgetChild;
        }
    }
    ChildAttr::None
}

fn impl_widget_macro(ast: &syn::DeriveInput) -> TokenStream {
    let trait_type = if let Some(attr) = ast.attrs.iter().find(|attr| {
        attr.path.segments.first().map(|i| i.ident == "widget_children_trait").unwrap_or(false)
    }) {
        let mut out = None;
        for token in attr.tokens.clone().into_iter() {
            match token {
                proc_macro2::TokenTree::Group(grp) => {
                    out = Some(grp.stream());
                    break;
                }
                _ => {}
            }
        }

        out
    } else {
        None
    }
    .unwrap_or(quote! { reclutch::widget::WidgetChildren });

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let name = &ast.ident;
    let mut children = Vec::new();

    let mut capacity = 0;
    if let syn::Data::Struct(ref data) = &ast.data {
        match &data.fields {
            syn::Fields::Named(fields) => {
                for field in fields.named.iter() {
                    if let Some(ref ident) = field.ident {
                        match chk_attrs_is_child(&field.attrs) {
                            ChildAttr::None => continue,
                            ChildAttr::WidgetChild => {
                                capacity += 1;
                                children.push(ChildReference::Single(StringOrInt::String(
                                    ident.to_string(),
                                )));
                            }
                            ChildAttr::VecWidgetChild => {
                                children.push(ChildReference::Vec(StringOrInt::String(
                                    ident.to_string(),
                                )));
                            }
                        }
                    }
                }
            }
            syn::Fields::Unnamed(fields) => {
                for (i, field) in fields.unnamed.iter().enumerate() {
                    match chk_attrs_is_child(&field.attrs) {
                        ChildAttr::None => continue,
                        ChildAttr::WidgetChild => {
                            capacity += 1;
                            children.push(ChildReference::Single(StringOrInt::Int(i)));
                        }
                        ChildAttr::VecWidgetChild => {
                            children.push(ChildReference::Vec(StringOrInt::Int(i)));
                        }
                    }
                }
            }
            _ => (),
        }
    }

    let mut push_children = Vec::new();
    let mut push_children_mut = Vec::new();
    let mut capacities = Vec::new();

    for child in children {
        match child {
            ChildReference::Single(ident) => match ident {
                StringOrInt::String(child) => {
                    let ident = quote::format_ident!("{}", child);
                    push_children.push(quote! { children.push(&self.#ident as _); });
                    push_children_mut.push(quote! { children.push(&mut self.#ident as _); });
                }
                StringOrInt::Int(child) => {
                    let ident = syn::Index::from(child);
                    push_children.push(quote! { children.push(&self.#ident as _); });
                    push_children_mut.push(quote! { children.push(&mut self.#ident as _); });
                }
            },
            ChildReference::Vec(ident) => match ident {
                StringOrInt::String(child) => {
                    let ident = quote::format_ident!("{}", child);
                    push_children
                        .push(quote! { for child in &self.#ident { children.push(child as _); } });
                    push_children_mut.push(
                        quote! { for child in &mut self.#ident { children.push(child as _); } },
                    );
                    capacities.push(quote! { + self.#ident.len() });
                }
                StringOrInt::Int(child) => {
                    let ident = syn::Index::from(child);
                    push_children
                        .push(quote! { for child in &self.#ident { children.push(child as _); } });
                    push_children_mut.push(
                        quote! { for child in &mut self.#ident { children.push(child as _); } },
                    );
                    capacities.push(quote! { + self.#ident.len() });
                }
            },
        }
    }

    {
        quote! {
            impl #impl_generics #trait_type for #name #ty_generics #where_clause {
                fn children(
                    &self
                ) -> Vec<
                    &dyn #trait_type<
                        UpdateAux = Self::UpdateAux,
                        GraphicalAux = Self::GraphicalAux,
                        DisplayObject = Self::DisplayObject,
                    >
                > {
                    let mut children = Vec::with_capacity(#capacity as usize #(#capacities)*);
                    #(#push_children)*
                    children
                }
                fn children_mut(
                    &mut self
                ) -> Vec<
                    &mut dyn #trait_type<
                        UpdateAux = Self::UpdateAux,
                        GraphicalAux = Self::GraphicalAux,
                        DisplayObject = Self::DisplayObject,
                    >
                > {
                    let mut children = Vec::with_capacity(#capacity as usize #(#capacities)*);
                    #(#push_children_mut)*
                    children
                }
            }
        }
    }
    .into()
}

#[proc_macro_derive(Event, attributes(event_key))]
pub fn event_macro_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    event::impl_event_macro(ast)
}
