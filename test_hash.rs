use git2::Oid;
fn main() {
    let hash1 = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e";
    let hash2 = "9d25845c91ff1aac84dbffd96664d8d6c16dccb2f";
    let hash3 = "ae36956d02aa2bce95ecbba07775e9e7d27edde3a";
    
    println\!("Hash1 length: {}, valid: {:?}", hash1.len(), Oid::from_str(hash1));
    println\!("Hash2 length: {}, valid: {:?}", hash2.len(), Oid::from_str(hash2));
    println\!("Hash3 length: {}, valid: {:?}", hash3.len(), Oid::from_str(hash3));
}
EOF < /dev/null