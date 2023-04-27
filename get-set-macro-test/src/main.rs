use std::collections::HashSet;
use get_set_macro::GetSet;

#[derive(GetSet)]
pub struct Test {
    field1: String,
    field2: i32,
    labels: HashSet<String>,
    string: String,
    field3: i32,
}

fn main() {
    let test = Test::new();

    assert_eq!(test.get_val::<String>("field1"), Ok("".to_string()));
    assert_eq!(test.get_val::<i32>("field2"), Ok(0));
    assert!(test.get_val::<String>("field3").is_err());

    let mut test = test;

    assert!(test.set_val::<String>("string", String::from("hello")).is_ok());
    assert_eq!(test.get_val::<String>("string"), Ok("hello".to_string()));

    assert!(test.set_val::<HashSet<String>>("labels", HashSet::from([
        "l1".to_string(),
        "l2".to_string(),
        "l3".to_string(),
        "l4".to_string(),
        "l5".to_string(),
    ])).is_ok());

    assert_eq!(test.get_val::<HashSet<String>>("labels").unwrap().len(), 5);
}