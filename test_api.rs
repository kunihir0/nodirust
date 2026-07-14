use p256::SecretKey;
fn main() {
    let mut rng = rand::rng();
    let secret = SecretKey::generate(&mut rng);
}
