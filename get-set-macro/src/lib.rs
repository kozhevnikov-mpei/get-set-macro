extern crate proc_macro;
use quote::ToTokens;
use proc_macro2::TokenStream;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use quote::quote;
use syn::{Data, DeriveInput, Ident, parse_macro_input};
use syn::Type as SynType;

#[proc_macro_derive(GetSet)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // вынимаю данные
    let input = parse_macro_input!(input as DeriveInput);
    let data = if let Data::Struct(ref data) = input.data {
        data
    } else {
        panic!("Only struct types are supported");
    };

    // генерирую вспомогательный enum и набор всех полей
    let enum_name = format!("{}InputType", input.ident.to_string());
    let fields = data.fields.iter()
        .map(|field| (field.ident.clone().unwrap(), Type::from(&field.ty)))
        .collect::<HashMap<_, _>>();
    let mut tokens = quote! {};

    generate_default(&input, &fields, &mut tokens);
    generate_enum(&enum_name, &fields, &mut tokens );

    let field_types = fields.iter()
        .map(|(_, type_)| type_)
        .collect::<HashSet<_>>();

    for field_type in field_types {
        generate_enum_try_from(&enum_name, &fields, field_type, &mut tokens);
        generate_enum_try_into(&enum_name, &fields, field_type, &mut tokens);
    }

    generate_impl(&input, &enum_name, &fields, &mut tokens);
    tokens.into()
}

/// генерация имплементации Default
fn generate_default(input: &DeriveInput, fields: &HashMap<Ident, Type>, tokens: &mut TokenStream) {
    let name = &input.ident;

    let fields_str = fields.iter()
        .map(|(field, _)| format!("{}: Default::default(), ", field))
        .collect::<String>();
    let fields = create_tokens(&format!(r"
        impl Default for {name} {{
         fn default() -> Self {{
          Self {{
           {fields_str}
          }}
         }}
        }}"));

    tokens.extend(quote! {
        #fields
    });
}

/// генерация вспомогательного enum со всеми полями
fn generate_enum(name: &str, fields: &HashMap<Ident, Type>, tokens: &mut TokenStream) {
    let enum_variants = fields.iter()
        .map(|(ident, type_)| format!("{}({}),", &ident.to_string(), type_ ))
        .collect::<String>();
    let enum_tokens = create_tokens(&format!(r"
        pub enum {name} {{
         {enum_variants}
        }}"));

    tokens.extend(quote! {
        #enum_tokens
    });
}

/// генерация имплементации TryFrom для этого enum + использование имени в качестве маркера для поля
fn generate_enum_try_from(enum_name: &str, fields: &HashMap<Ident, Type>, field_type: &Type, tokens: &mut TokenStream) {
    let mapping = fields.iter()
        .map(|(field, type_)| {
            let field_name = field.to_string();
            if type_ == field_type {
                format!("\"{field_name}\" => Ok({enum_name}::{field_name}(value.1)),")
            } else {
                format!("\"{field_name}\" => Err(\"Wrong type: got {type_} when expected {field_type}\".to_string()),")
            }
        })
        .collect::<String>();
    let try_from_type = format!(r#"
       impl TryFrom<(&'static str, {field_type})> for {enum_name} {{
            type Error = String;

            fn try_from(value: (&'static str, {field_type})) -> Result<Self, Self::Error> {{
                match value.0 {{
                    {mapping}
                    field => Err(format!("Unknown field {{}}", field)),
                }}
            }}
       }}
    "#);

    let try_from_type = create_tokens(&try_from_type);

    tokens.extend(quote! {
        #try_from_type
    });
}

/// генерация имплементации TryFrom для типов, известных внутри enum
fn generate_enum_try_into(enum_name: &str, fields: &HashMap<Ident, Type>, field_type: &Type, tokens: &mut TokenStream) {
    let mapping = fields.iter()
        .map(|(field, type_)| {
            let field_name = field.to_string();
            if type_ == field_type {
                format!("{enum_name}::{field_name}(inner) => Ok(inner),")
            } else {
                format!("{enum_name}::{field_name}(_) => Err(\"Wrong type: {enum_name} contains {field_type} when expected {type_}\".to_string()),")
            }
        })
        .collect::<String>();
    let try_from_type = format!(r#"
       impl TryFrom<{enum_name}> for {field_type} {{
            type Error = String;

            fn try_from(value: {enum_name}) -> Result<Self, Self::Error> {{
                match value {{
                    {mapping}
                }}
            }}
       }}
    "#);

    let try_from_type = create_tokens(&try_from_type);

    tokens.extend(quote! {
        #try_from_type
    });
}

/// генерация имплементации методов new, get_val и set_val
fn generate_impl(input: &DeriveInput, enum_name: &str, fields: &HashMap<Ident, Type>, tokens: &mut TokenStream) {
    let name = input.ident.to_string();
    let impl_get = generate_impl_get(enum_name, fields);
    let impl_set = generate_impl_set(enum_name, fields);
    let impl_str = create_tokens(&format!(r"
        impl {name} {{
            pub fn new() -> Self {{
                Self::default()
            }}

            {impl_get}

            {impl_set}
        }}
    "));

    tokens.extend(quote! {
        #impl_str
    })
}

/// генерация геттера
fn generate_impl_get(enum_name: &str, fields: &HashMap<Ident, Type>) -> String {
    let mapping = fields.iter()
        .map(|(field, _)| {
            let field_name = field.to_string();
            format!("\"{field_name}\" => {enum_name}::try_from((\"{field_name}\", self.{field_name}.clone())),")
        })
        .collect::<String>();

    let impl_str = format!(r#"
        pub fn get_val<T>(&self, field_name: &'static str) -> Result<T, String>
        where T: TryFrom<{enum_name}, Error=String> {{
            let enum_value = match field_name {{
                {mapping}
                _ => return Err(format!("Unknown field {{}}", field_name)),
            }}?;

            enum_value.try_into()
        }}
    "#);
    impl_str
}

/// генерация сеттера
fn generate_impl_set(enum_name: &str, fields: &HashMap<Ident, Type>) -> String {
    let mapping = fields.iter()
        .map(|(field, _)| {
            let field_name = field.to_string();
            format!("(\"{field_name}\", {enum_name}::{field_name}(inner)) => self.{field_name} = inner,")
        })
        .collect::<String>();

    let impl_str = format!(r#"
        pub fn set_val<T>(&mut self, field_name: &'static str, value: T) -> Result<(), String>
        where {enum_name}: TryFrom<(&'static str, T), Error=String> {{
            let enum_value = {enum_name}::try_from((field_name, value))?;
            match (field_name, enum_value) {{
                {mapping}
                _ => return Err(format!("Can't set {{}}", field_name)),
            }};
            Ok(())
        }}
    "#);
    impl_str
}

/// вспомогательная функция-обертка
fn create_tokens(text: &str) -> TokenStream {
    proc_macro::TokenStream::from_str(text).unwrap().into()
}

#[derive(PartialEq, Eq, Clone, Hash)]
/// пришлось создать свой тип для замены [syn::Type],
/// чтобы сравнивать типы, выводить, считать хеш и клонировать
struct Type(String);

impl From<&SynType> for Type {
    fn from(t: &SynType) -> Self {
        Self(t.to_token_stream().to_string().replace(" ", ""))
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
