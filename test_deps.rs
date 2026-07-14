use p256::SecretKey;
use p256::elliptic_curve::sec1::ToSec1Point;
fn main() {
    let mut rng = rand::rng();
    let secret = SecretKey::random(&mut rng);
    let public = secret.public_key();
    let encoded = public.to_sec1_point(false);
}
