extern crate proc_macro;

use {proc_macro::TokenStream, quote::quote};

#[proc_macro_derive(WidgetChildren, attributes(widget_child))]
pub fn widget_macro_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_widget_macro(&ast)
}

fn chk_attrs_is_child(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr
            .path
            .segments
            .first()
            .map(|i| i.ident == "widget_child")
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn impl_widget_macro(ast: &syn::DeriveInput) -> TokenStream {
    enum StringOrInt {
        String(String),
        Int(usize),
    }

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let name = &ast.ident;
    let mut children = Vec::new();

    if let syn::Data::Struct(ref data) = &ast.data {
        match &data.fields {
            syn::Fields::Named(fields) => {
                for field in fields.named.iter() {
                    if let Some(ref ident) = field.ident {
                        if chk_attrs_is_child(&field.attrs) {
                            children.push(StringOrInt::String(ident.to_string()));
                        }
                    }
                }
            }
            syn::Fields::Unnamed(fields) => {
                for (i, field) in fields.unnamed.iter().enumerate() {
                    if chk_attrs_is_child(&field.attrs) {
                        children.push(StringOrInt::Int(i));
                    }
                }
            }
            _ => (),
        }
    }

    let (children, mut_children): (Vec<_>, Vec<_>) = children
        .iter()
        .map(|child| match child {
            StringOrInt::String(child) => {
                let ident = quote::format_ident!("{}", child);
                (quote! { &self.#ident }, quote! { &mut self.#ident })
            }
            StringOrInt::Int(child) => {
                let ident = syn::Index::from(*child);
                (quote! { &self.#ident }, quote! { &mut self.#ident })
            }
        })
        .unzip();

    {
        quote! {
            impl #impl_generics reclutch::widget::WidgetChildren for #name #ty_generics #where_clause {
                fn children(
                    &self
                ) -> Vec<
                    &dyn reclutch::widget::WidgetChildren<
                        UpdateAux = Self::UpdateAux,
                        GraphicalAux = Self::GraphicalAux,
                        DisplayObject = Self::DisplayObject,
                    >
                > {
                    vec![ #(#children),* ]
                }
                fn children_mut(
                    &mut self
                ) -> Vec<
                    &mut dyn reclutch::widget::WidgetChildren<
                        UpdateAux = Self::UpdateAux,
                        GraphicalAux = Self::GraphicalAux,
                        DisplayObject = Self::DisplayObject,
                    >
                > {
                    vec![ #(#mut_children),* ]
                }
            }
        }
    }.into()
}
