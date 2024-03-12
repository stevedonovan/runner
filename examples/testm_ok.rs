{ let mut map: std::collections::HashMap<String,String> = HashMap::new();
    map.insert("hello".to_string(),"world".to_string());
    let option = map.insert("hello".to_string(),"dolly".to_string());
    if let Some(ref previous) = option {
        assert!(previous == "world");
        option
    } else { None }
}
