
fn main() {
    let es_aff = include_str!("../dictionaries/es_ES.aff");
    let es_dic = include_str!("../dictionaries/es_ES.dic");
    println!("Parsing es_ES...");
    match spellbook::Dictionary::new(es_aff, es_dic) {
        Ok(dict) => {
            println!("es_ES loaded successfully!");
            println!("hola: {}", dict.check("hola"));
            println!("química: {}", dict.check("química"));
            println!("casa: {}", dict.check("casa"));
            println!("erróneo: {}", dict.check("erróneo"));
            println!("xyzabc: {}", dict.check("xyzabc"));
            let mut sugg = Vec::new();
            dict.suggest("quimica", &mut sugg);
            println!("suggestions for quimica: {:?}", sugg);
        }
        Err(e) => {
            println!("Error loading es_ES: {:?}", e);
        }
    }

    let en_aff = include_str!("../dictionaries/en_US.aff");
    let en_dic = include_str!("../dictionaries/en_US.dic");
    println!("Parsing en_US...");
    match spellbook::Dictionary::new(en_aff, en_dic) {
        Ok(dict) => {
            println!("en_US loaded successfully!");
            println!("hello: {}", dict.check("hello"));
            println!("chemistry: {}", dict.check("chemistry"));
        }
        Err(e) => {
            println!("Error loading en_US: {:?}", e);
        }
    }
}
