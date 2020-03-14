use std::thread;
use hex;
use base58::{ToBase58};
use bech32::{Bech32, u5, ToBase32};
use rand::{Rng, ChaChaRng, FromEntropy, SeedableRng};
use json::{array, object};
use sha2::{Sha256, Digest};
use std::io;
use std::io::Write;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::panic;
use std::time::{SystemTime};
use zcash_primitives::zip32::{DiversifierIndex, DiversifierKey, ChildIndex, ExtendedSpendingKey, ExtendedFullViewingKey};

/// A trait for converting a [u8] to base58 encoded string.
pub trait ToBase58Check {
    /// Converts a value of `self` to a base58 value, returning the owned string.
    /// The version is a coin-specific prefix that is added. 
    /// The suffix is any bytes that we want to add at the end (like the "iscompressed" flag for 
    /// Secret key encoding)
    fn to_base58check(&self, version: &[u8], suffix: &[u8]) -> String;
}

impl ToBase58Check for [u8] {
    fn to_base58check(&self, version: &[u8], suffix: &[u8]) -> String {
        let mut payload: Vec<u8> = Vec::new();
        payload.extend_from_slice(version);
        payload.extend_from_slice(self);
        payload.extend_from_slice(suffix);
        
        let mut checksum = double_sha256(&payload);
        payload.append(&mut checksum[..4].to_vec());
        payload.to_base58()
    }
}

/// Sha256(Sha256(value))
pub fn double_sha256(payload: &[u8]) -> Vec<u8> {
    let h1 = Sha256::digest(&payload);
    let h2 = Sha256::digest(&h1);
    h2.to_vec()
}

/// Parameters used to generate addresses and private keys. Look in chainparams.cpp (in zcashd/src)
/// to get these values. 
/// Usually these will be different for testnet and for mainnet.
pub struct CoinParams {
    pub taddress_version: [u8; 2],
    pub tsecret_prefix  : [u8; 1],
    pub zaddress_prefix : String,
    pub zsecret_prefix  : String,
    pub zviewkey_prefix : String,
    pub cointype        : u32,
}

pub fn params() -> CoinParams {
        CoinParams {
            taddress_version : [0x1C, 0xB8],
            tsecret_prefix   : [0x80],
            zaddress_prefix  : "zs".to_string(),
            zsecret_prefix   : "secret-extended-key-main".to_string(),
            zviewkey_prefix  : "zviews".to_string(),
            cointype         : 133
        }
}

pub fn increment(s: &mut [u8; 32]) -> Result<(), ()> {
    for k in 0..32 {
        s[k] = s[k].wrapping_add(1);
        if s[k] != 0 {
            // No overflow
            return Ok(());
        }
    }
    // Overflow
    Err(())
}

// Turn the prefix into Vec<u5>, so it can be matched directly without any encoding overhead.
fn get_bech32_for_prefix(prefix: String) -> Result<Vec<u5>, String> {
    // Reverse character set. Maps ASCII byte -> CHARSET index on [0,31]
    const CHARSET_REV: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        15, -1, 10, 17, 21, 20, 26, 30,  7,  5, -1, -1, -1, -1, -1, -1,
        -1, 29, -1, 24, 13, 25,  9,  8, 23, -1, 18, 22, 31, 27, 19, -1,
        1,  0,  3, 16, 11, 28, 12, 14,  6,  4,  2, -1, -1, -1, -1, -1,
        -1, 29, -1, 24, 13, 25,  9,  8, 23, -1, 18, 22, 31, 27, 19, -1,
        1,  0,  3, 16, 11, 28, 12, 14,  6,  4,  2, -1, -1, -1, -1, -1
    ];

    let mut ans = Vec::new();
    for c in prefix.chars() {
        if CHARSET_REV[c as usize] == -1 {
            return Err(format!("Invalid character in prefix: '{}'", c));
        }
        ans.push(u5::try_from_u8(CHARSET_REV[c as usize] as u8).expect("Should be able to convert to u5"));
    }

    return Ok(ans);
}

fn encode_address(spk: &ExtendedSpendingKey) -> String {
    let (_d, addr) = spk.default_address().expect("Cannot get result");

    // Address is encoded as a bech32 string
    let mut v = vec![0; 43];

    v.get_mut(..11).unwrap().copy_from_slice(&addr.diversifier.0);
    addr.pk_d.write(v.get_mut(11..).unwrap()).expect("Cannot write!");
    let checked_data: Vec<u5> = v.to_base32();
    let encoded : String = Bech32::new(params().zaddress_prefix.into(), checked_data).expect("bech32 failed").to_string();
    
    return encoded;
}

fn encode_privatekey(spk: &ExtendedSpendingKey) -> String {
    // Private Key is encoded as bech32 string
    let mut vp = Vec::new();
    spk.write(&mut vp).expect("Can't write private key");
    let c_d: Vec<u5> = vp.to_base32();
    let encoded_pk = Bech32::new(params().zsecret_prefix.into(), c_d).expect("bech32 failed").to_string();

    return encoded_pk;
}

/// A single thread that grinds through the Diversifiers to find the defualt key that matches the prefix
pub fn vanity_thread(entropy: &[u8], prefix: String, tx: mpsc::Sender<String>, please_stop: Arc<AtomicBool>) {
    
    let mut seed: [u8; 32] = [0; 32];
    seed.copy_from_slice(&entropy[0..32]);

    let di = DiversifierIndex::new();
    let vanity_bytes = get_bech32_for_prefix(prefix).expect("Bad char in prefix");

    let master_spk = ExtendedSpendingKey::from_path(&ExtendedSpendingKey::master(&seed),
                            &[ChildIndex::Hardened(32), ChildIndex::Hardened(params().cointype), ChildIndex::Hardened(0)]);

    let mut spkv = vec![];
    master_spk.write(&mut spkv).unwrap();

    let mut i: u32 = 0;
    loop {
        if increment(&mut seed).is_err() {
            return;
        }

        let dk = DiversifierKey::master(&seed);
        let (_ndk, nd) = dk.diversifier(di).unwrap();

        // test for nd
        let mut isequal = true;
        for i in 0..vanity_bytes.len() {
            if vanity_bytes[i] != nd.0.to_base32()[i] {
                isequal = false;
                break;
            }
        }

        if isequal { 
            let len = spkv.len();
            spkv[(len-32)..len].copy_from_slice(&dk.0[0..32]);
            let spk = ExtendedSpendingKey::read(&spkv[..]).unwrap();

            
            let encoded = encode_address(&spk);
            let encoded_pk = encode_privatekey(&spk);
            
            let wallet = array!{object!{
                "num"           => 0,
                "address"       => encoded,
                "private_key"   => encoded_pk,
                "type"          => "zaddr"}};
            
            tx.send(json::stringify_pretty(wallet, 2)).unwrap();
            return;
        }

        i = i + 1;
        if i%5000 == 0 {
            if please_stop.load(Ordering::Relaxed) {
                return;
            }
            tx.send("Processed:5000".to_string()).unwrap();
        }

        if i == 0 { return; }
    }
}

fn pretty_duration(secs: f64) -> (String, String) {
    let mut expected_dur  = "sec";
    let mut expected_time = secs;

    if expected_time > 60.0 {
        expected_time /= 60.0;
        expected_dur = "min";
    }
    if expected_time > 60.0 {
        expected_time /= 60.0;
        expected_dur = "hours";
    }
    if expected_time > 24.0 {
        expected_time /= 24.0;
        expected_dur = "days";
    }
    if expected_time > 30.0 {
        expected_time /= 30.0;
        expected_dur = "months";
    }
    if expected_time > 12.0 {
        expected_time /= 12.0;
        expected_dur = "years";
    }

    return (format!("{:.*}", 0, expected_time), expected_dur.to_string());
}

/// Generate a vanity address with the given prefix.
pub fn generate_vanity_wallet(num_threads: u32, prefix: String) -> Result<String, String> {
    // Test the prefix first
    match get_bech32_for_prefix(prefix.clone()) {
        Ok(_)  => (),
        Err(e) => return Err(format!("{}. Note that ['b', 'i', 'o', '1'] are not allowed in addresses.", e))
    };

    // Get 32 bytes of system entropy
    let mut system_rng = ChaChaRng::from_entropy();    
    
    let (tx, rx) = mpsc::channel();
    let please_stop = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::new();

    for _i in 0..num_threads {
        //let testnet_local = is_testnet.clone();
        let prefix_local = prefix.clone();
        let tx_local = mpsc::Sender::clone(&tx);
        let ps_local = please_stop.clone();
    
        let mut entropy: [u8; 32] = [0; 32];
        system_rng.fill(&mut entropy);
    
        let handle = thread::spawn(move || {
            vanity_thread(&entropy, prefix_local, tx_local, ps_local);
        });
        handles.push(handle);
    }
    
    let mut processed: u64   = 0;
    let now = SystemTime::now();

    let wallet: String;

    // Calculate the estimated time
    let expected_combinations = (32 as f64).powf(prefix.len() as f64);

    loop {
        let recv = rx.recv().unwrap();
        if recv.starts_with(&"Processed") {
            processed = processed + 5000;
            let timeelapsed = now.elapsed().unwrap().as_secs() + 1; // Add one second to prevent any divide by zero problems.

            let rate = processed / timeelapsed;            
            let expected_secs = expected_combinations / (rate as f64);

            let (s, d) = pretty_duration(expected_secs);

            print!("Checking addresses at {}/sec on {} CPU threads. [50% ETA = {} {}]   \r", rate, num_threads, s, d);
            io::stdout().flush().ok().unwrap();
        } else {
            // Found a solution
            println!("");   // To clear the previous inline output to stdout;
            wallet = recv;

            please_stop.store(true, Ordering::Relaxed);
            break;
        } 
    }

    for handle in handles {
        handle.join().unwrap();
    }    

    return Ok(wallet);
}

/// Generate a series of `count` addresses and private keys. 
pub fn generate_wallet(nohd: bool, zcount: u32, tcount: u32, user_entropy: &[u8]) -> String {        
    // Get 32 bytes of system entropy
    let mut system_entropy:[u8; 32] = [0; 32]; 
    {
        let result = panic::catch_unwind(|| {
            ChaChaRng::from_entropy()
        });

        let mut system_rng = match result {
            Ok(rng)     => rng,
            Err(_e)     => ChaChaRng::from_seed([0; 32])
        };

        system_rng.fill(&mut system_entropy);
    }    
    
    // Add in user entropy to the system entropy, and produce a 32 byte hash... 
    let mut state = sha2::Sha256::new();
    state.input(&system_entropy);
    state.input(&user_entropy);
    
    let mut final_entropy: [u8; 32] = [0; 32];
    final_entropy.clone_from_slice(&double_sha256(&state.result()[..]));

    // ...which will we use to seed the RNG
    let mut rng = ChaChaRng::from_seed(final_entropy);

    if !nohd {
        // Allow HD addresses, so use only 1 seed        
        let mut seed: [u8; 32] = [0; 32];
        rng.fill(&mut seed);
        
        return gen_addresses_with_seed_as_json(zcount, tcount, |i| (seed.to_vec(), i));
    } else {
        // Not using HD addresses, so derive a new seed every time    
        return gen_addresses_with_seed_as_json(zcount, tcount, |_| {            
            let mut seed:[u8; 32] = [0; 32]; 
            rng.fill(&mut seed);
            
            return (seed.to_vec(), 0);
        });
    }    
}

/// Generate `count` addresses with the given seed. The addresses are derived from m/32'/cointype'/index' where 
/// index is 0..count
/// 
/// Note that cointype is 1 for testnet and 133 for mainnet
/// 
/// get_seed is a closure that will take the address number being derived, and return a tuple cointaining the 
/// seed and child number to use to derive this wallet. 
/// It is useful if we want to reuse (or not) the seed across multiple wallets.
fn gen_addresses_with_seed_as_json<F>(zcount: u32, tcount: u32, mut get_seed: F) -> String 
    where F: FnMut(u32) -> (Vec<u8>, u32)
{
    let mut ans = array![];

    // Note that for t-addresses, we don't use HD addresses
    let (seed, _) = get_seed(0);
    let mut rng_seed: [u8; 32] = [0; 32];
    rng_seed.clone_from_slice(&seed[0..32]);
    
    // First generate the Z addresses
    for i in 0..zcount {
        let (seed, child) = get_seed(i);
        let (addr, pk, _vk, path) = get_zaddress(&seed, child);
        ans.push(object!{
                "num"           => i,
                "address"       => addr,
                "private_key"   => pk,
                "type"          => "zaddr",
                "seed"          => path
        }).unwrap(); 
    }      

    // Next generate the T addresses
    // derive a RNG from the seed
    let mut rng = ChaChaRng::from_seed(rng_seed);

    for i in 0..tcount {        
        let (addr, pk_wif) = get_taddress(&mut rng);

        ans.push(object!{
            "num"               => i,
            "address"           => addr,
            "private_key"       => pk_wif,
            "type"              => "taddr"
        }).unwrap();
    }

    return json::stringify_pretty(ans, 2);
}

/// Generate a t address
fn get_taddress(rng: &mut ChaChaRng) -> (String, String) {
//    use secp256k1;
    use ripemd160::{Ripemd160};

    let mut sk_bytes: [u8; 32] = [0;32];

    // There's a small chance the generated private key bytes are invalid, so
    // we loop till we find bytes that are
    let sk = loop {    
        rng.fill(&mut sk_bytes);

        match secp256k1::SecretKey::parse(&sk_bytes) {
            Ok(s)  => break s,
            Err(_) => continue
        }
    };
    
    let pubkey = secp256k1::PublicKey::from_secret_key(&sk);

    // Address 
    let mut hash160 = Ripemd160::new();
    hash160.input(sha2::Sha256::digest(&pubkey.serialize_compressed().to_vec()));
    let addr = hash160.result().to_base58check(&params().taddress_version, &[]);

    // Private Key
    let pk_wif = sk_bytes.to_base58check(&params().tsecret_prefix, &[0x01]);  

    return (addr, pk_wif);
}

/// Generate a standard ZIP-32 address from the given seed at 32'/44'/0'/index
fn get_zaddress(seed: &[u8], index: u32) -> (String, String, String, json::JsonValue) {
   let spk: ExtendedSpendingKey = ExtendedSpendingKey::from_path(
            &ExtendedSpendingKey::master(seed),
            &[
                ChildIndex::Hardened(32),
                ChildIndex::Hardened(params().cointype),
                ChildIndex::Hardened(index)
            ],
        );
    let path = object!{
        "HDSeed"    => hex::encode(seed),
        "path"      => format!("m/32'/{}'/{}'", params().cointype, index)
    };

    let encoded = encode_address(&spk);
    let encoded_pk = encode_privatekey(&spk);

    // Viewing Key is encoded as bech32 string
    let mut vv = Vec::new();
    ExtendedFullViewingKey::from(&spk).write(&mut vv).expect("Can't write viewing key");
    let c_v: Vec<u5> = vv.to_base32();
    let encoded_vk = Bech32::new(params().zviewkey_prefix.into(), c_v).expect("bech32 failed").to_string();

    return (encoded, encoded_pk, encoded_vk, path);
}






// Tests
#[cfg(test)]
mod tests {
    
    /// Test the wallet generation and that it is generating the right number and type of addresses
    #[test]
    fn test_wallet_generation() {
        use crate::paper::generate_wallet;
        use std::collections::HashSet;

        // Mainnet wallet
        let w = generate_wallet(false, 1, 0, &[]);
        let j = json::parse(&w).unwrap();
        assert_eq!(j.len(), 1);
        assert!(j[0]["address"].as_str().unwrap().starts_with("zs"));
        assert!(j[0]["private_key"].as_str().unwrap().starts_with("secret-extended-key-main"));
        assert_eq!(j[0]["seed"]["path"].as_str().unwrap(), "m/32'/133'/0'");

        // Check if all the addresses are the same
        let w = generate_wallet(false, 3, 0, &[]);
        let j = json::parse(&w).unwrap();
        assert_eq!(j.len(), 3);

        let mut set1 = HashSet::new();
        let mut set2 = HashSet::new();
        for i in 0..3 {
            assert!(j[i]["address"].as_str().unwrap().starts_with("ztestsapling"));
            assert_eq!(j[i]["seed"]["path"].as_str().unwrap(), format!("m/32'/1'/{}'", i).as_str());

            set1.insert(j[i]["address"].as_str().unwrap());
            set1.insert(j[i]["private_key"].as_str().unwrap());

            set2.insert(j[i]["seed"]["HDSeed"].as_str().unwrap());
        }

        // There should be 3 + 3 distinct addresses and private keys
        assert_eq!(set1.len(), 6);
        // ...but only 1 seed
        assert_eq!(set2.len(), 1);
    }

    #[test]
    fn test_z_encoding() {
        use crate::paper::{encode_address, encode_privatekey};
        use zcash_primitives::zip32::ExtendedSpendingKey;

        let main_data = "[
            {'encoded' : '037d54cb810000008079a0d98ee64814bffe3f78e0b67363bdcdfd57b6a9a8f871615884ef79a001fdc59be1b24f5d75beed619d2eb3722a5f7f9d9c9e13f6c0218cd10bffe5ec0c0b21d65ad27ac913dfcd2d40425345d49c09e4fed60555a5f3346d76ed45906004f4c2cc6098f0780b9adaa0b1636976dcd8d6311812ef42f073d506ae19bbe4ff7501070410c512af68ed0141e146c69af666fe2efdeb804df33e3304ce07a0bb', 'address' : 'zs1ttwlzs7nnmdwmx7eag3k4szxzvsa82ttsakmux5zk0y9vcqp4jguecn5rqkjjdae2pgzcta4vkt', 'pk' : 'secret-extended-key-main1qd74fjupqqqqpqre5rvcaejgzjllu0mcuzm8xcaaeh740d4f4ru8zc2csnhhngqplhzehcdjfawht0hdvxwjavmj9f0hl8vuncfldspp3ngshll9asxqkgwkttf84jgnmlxj6szz2dzaf8qfunldvp245hengmtka4zeqcqy7npvccyc7puqhxk65zckx6tkmnvdvvgczth59urn65r2uxdmunlh2qg8qsgv2y40drkszs0pgmrf4anxlch0m6uqfhenuvcyecr6pwcvt7qwu'}, 
            {'encoded' : '03747bda750000008090dd234894f208a53bec30461e9a1abe6c9ecce833b2110132576d4b135dee0cd328312ba73ae04a05e79fd81ba7d57bb4bc0a9a7a7a11ca904b604f9be62f0ea011906ac33e3dbbc0983228ed3c334373873d6bc309054c24538c93c3677e0332c848dadbee9308fe0d37241aa6e34541e3837a272a4d08e30ac1470ef389c46370ae1ca72bb87488bcfa8cb26040604ef3dd8c2a8590e3f05ee771ba6d7e89', 'address' : 'zs1ttryt8fh0hu74upauprpglddcm3avmclnr2ywsxzhpqgchcd29xyqtvpqx7wktvx94cg6522ldy', 'pk' : 'secret-extended-key-main1qd68hkn4qqqqpqysm535398jpzjnhmpsgc0f5x47dj0ve6pnkggszvjhd493xh0wpnfjsvft5uawqjs9u70asxa864amf0q2nfa85yw2jp9kqnumuchsagq3jp4vx03ah0qfsv3ga57rxsmnsu7khscfq4xzg5uvj0pkwlsrxtyy3kkma6fs3lsdxujp4fhrg4q78qm6yu4y6z8rptq5wrhn38zxxu9wrjnjhwr53z704r9jvpqxqnhnmkxz4pvsu0c9aem3hfkhazgksps0h'}
        ]";

        let j = json::parse(&main_data.replace("'", "\"")).unwrap();
        for i in j.members() {
            let e = hex::decode(i["encoded"].as_str().unwrap()).unwrap();
            let spk = ExtendedSpendingKey::read(&e[..]).unwrap();

            assert_eq!(encode_address(&spk, false), i["address"]);
            assert_eq!(encode_privatekey(&spk, false), i["pk"]);
        }

        let test_data = "[
            {'encoded' : '03f577d7b800000080b9ae0ce9f44f7b3550e14f4662e91270b04b265ff4ba4546be72feef91b38d3397b3d25a79d67fa024a1b0d3f4d5143eff3e410c300bf615090dbdbddea6b70302bb8b73449cafa1ce1862bd4af31db2d468e39c451cfb026128ea3abe6b820ccb1b8e3a4e6faccef50f9f3c02a5cd55d9faebc4939d6d5f5271b8a66d73f443ec546c3cf583dccfed7994e856cd462a0a199cf6c89bdbe6b38c721dc07637ea', 'address' : 'ztestsapling1tsurvgycuy5me2nds2jpug806nr954ts3h3mf2de925qp8t9tyhvg0sfhe0qp3jf02vfxk3thn0', 'pk' : 'secret-extended-key-test1q06h04acqqqqpq9e4cxwnaz00v64pc20ge3wjynskp9jvhl5hfz5d0njlmhervudxwtm85j608t8lgpy5xcd8ax4zsl070jppscqhas4pyxmm0w756msxq4m3de5f890588psc4afte3mvk5dr3ec3gulvpxz28282lxhqsvevdcuwjwd7kvaag0nu7q9fwd2hvl467yjwwk6h6jwxu2vmtn73p7c4rv8n6c8hx0a4uef6zke4rz5zsennmv3x7mu6eccusacpmr06sjxk88k'},
            {'encoded' : '036b781dfd000000808956fba285802d5cebf5a24142c957877fa9a6182c57d24ab394e47eafc6c781750bcb2630ce11a90faf0e976d3898255a509e049d2332de9f332e254e91770ce45c085da9b55e108b5eaef45e68ab32bb9e461fe2356ea375258377044d190b1a630c1d1471d6cbc98b9e6dc779472a797d3cfcaf3dfbe5e878dbeae58e8a48347e48cf93de87f63aa3803556e9632e97a27374aef2988205ddcf69da12c95e', 'address' : 'ztestsapling1tscd2ap27tt4eg42m3k76ahg9gxgqf0lk8ls2tsxegkf7s050v9agccg0jg2s4ja4vkvccas270', 'pk' : 'secret-extended-key-test1qd4hs80aqqqqpqyf2ma69pvq94wwhadzg9pvj4u80756vxpv2lfy4vu5u3l2l3k8s96shjexxr8pr2g04u8fwmfcnqj455y7qjwjxvk7nuejuf2wj9mseezuppw6nd27zz94ath5te52kv4mnerplc34d63h2fvrwuzy6xgtrf3sc8g5w8tvhjvtnekuw7289fuh608u4u7lhe0g0rd74evw3fyrgljge7faaplk823cqd2ka93ja9azwd62au5csgzamnmfmgfvjhs68k0x5'},
            {'encoded' : '033d5066140000008099cfb65ab46e5a0e3f6891c1480fdb2f36f2fa02d75cfebb04e06513e4eaa148978f54f4e9fee05464a1574debae01ec1bd53c4c7ac4fd49414e4ab05b18a502c420031918f93c8756f054cdd134dabf36941b59f839761f2339b9d88a2d68073e53dce94d94c5118141179d1fb38f62705a3c1d27d2bb86bd0824cf72ac07d2095a13bd31975c706a7ec3e65310851363c658b76f3ac45484b4015ae93f0556', 'address' : 'ztestsapling1ts9afgw2k67qewv7wr08upf4wxe3m82u6fz432jpar7h48k60w4ksuereawhszsd0xvjyc5a5u0', 'pk' : 'secret-extended-key-test1qv74qes5qqqqpqyee7m94drwtg8r76y3c9yqlke0xme05qkhtnltkp8qv5f7f64pfztc7485a8lwq4ry59t5m6awq8kph4fuf3avfl2fg98y4vzmrzjs93pqqvv337fusat0q4xd6y6d40ekjsd4n7pewc0jxwdemz9z66q88efae62djnz3rq2pz7w3lvu0vfc950qaylfthp4apqjv7u4vqlfqjksnh5cewhrsdflv8ejnzzz3xc7xtzmk7wky2jztgq26ayls24srxx9hw'},
            {'encoded' : '03a19d13b700000080ff5f4ec78697bd786cb6dfe2e8cc57fd9cd4ad7f87bb9a92607cbf23122082e6c00e3eceb438a739738262e1ac3eabdb1d9c0a44b45b759939d159739b29880ba4437024a134269e16cd9a859f86854d5ea237e542f700805364a6d0515ac70a2fed943bef0430025c4d2895b780bbe08c659e37f3d60336c1cbc0bb17bb2488d7c6b55585b0743600826e333bd058b3fed68b02228efaa94b0f6eadf0fc7b68', 'address' : 'ztestsapling1ts8mqy2kvn7j3ktj9ean07tl0wktqnv6e5amrv92x2yenlx4hxc6tmktewc79mk0wlmkxh9fh4q', 'pk' : 'secret-extended-key-test1qwse6yahqqqqpq8lta8v0p5hh4uxedklut5vc4lann226lu8hwdfycruhu33ygyzumqqu0kwksu2wwtnsf3wrtp740d3m8q2gj69kave88g4juum9xyqhfzrwqj2zdpxnctvmx59n7rg2n275gm72shhqzq9xe9x6pg443c29lkegwl0qscqyhzd9z2m0q9muzxxt83h70tqxdkpe0qtk9amyjyd03442kzmqapkqzpxuvem6pvt8lkk3vpz9rh6499s7m4d7r78k6qa4j49t'}
        ]";

        let j = json::parse(&test_data.replace("'", "\"")).unwrap();
        for i in j.members() {
            let e = hex::decode(i["encoded"].as_str().unwrap()).unwrap();
            let spk = ExtendedSpendingKey::read(&e[..]).unwrap();

            assert_eq!(encode_address(&spk, true), i["address"]);
            assert_eq!(encode_privatekey(&spk, true), i["pk"]);
        }
    }

     #[test]
    fn test_entroy() {
        use crate::paper::generate_wallet;
        use crate::paper::generate_vanity_wallet;
        
        // Testnet wallet 1
        let w1 = generate_wallet(true, false, 1, 1, &[0; 32]);
        let j1 = json::parse(&w1).unwrap();
        assert_eq!(j1.len(), 2);

        // Testnet wallet 2, same user_entropy
        let w2 = generate_wallet(true, false, 1, 1, &[0; 32]);
        let j2 = json::parse(&w2).unwrap();
        assert_eq!(j2.len(), 2);

        // Make sure that the two addresses are different
        assert_ne!(j1[0]["address"].as_str().unwrap(), j2[0]["address"].as_str().unwrap());
        assert_ne!(j1[1]["address"].as_str().unwrap(), j2[1]["address"].as_str().unwrap());
        assert_ne!(j1[0]["private_key"].as_str().unwrap(), j2[0]["private_key"].as_str().unwrap());
        assert_ne!(j1[1]["private_key"].as_str().unwrap(), j2[1]["private_key"].as_str().unwrap());

        // Test the vanity address generator returns different addresses for every run
        let td1 = json::parse(&generate_vanity_wallet(false, 1, "te".to_string()).unwrap()).unwrap();
        let td2 = json::parse(&generate_vanity_wallet(false, 1, "te".to_string()).unwrap()).unwrap();
        assert!(td1[0]["address"].as_str().unwrap().starts_with("zs1te"));
        assert!(td2[0]["address"].as_str().unwrap().starts_with("zs1te"));

        assert_ne!(td1[0]["address"].as_str().unwrap(), td2[0]["address"].as_str().unwrap());
    }

    #[test]
    fn test_tandz_wallet_generation() {
        use crate::paper::generate_wallet;
        use std::collections::HashSet;
        
        // Testnet wallet
        let w = generate_wallet(true, false, 1, 1, &[]);
        let j = json::parse(&w).unwrap();
        assert_eq!(j.len(), 2);

        assert!(j[0]["address"].as_str().unwrap().starts_with("ztestsapling"));
        assert!(j[0]["private_key"].as_str().unwrap().starts_with("secret-extended-key-test"));
        assert_eq!(j[0]["seed"]["path"].as_str().unwrap(), "m/32'/1'/0'");

        assert!(j[1]["address"].as_str().unwrap().starts_with("tm"));
        let pk = j[1]["private_key"].as_str().unwrap();
        assert!(pk.starts_with("c") || pk.starts_with("9"));

        // Mainnet wallet
        let w = generate_wallet(false, false, 1, 1, &[]);
        let j = json::parse(&w).unwrap();
        assert_eq!(j.len(), 2);

        assert!(j[0]["address"].as_str().unwrap().starts_with("zs"));
        assert!(j[0]["private_key"].as_str().unwrap().starts_with("secret-extended-key-main"));
        assert_eq!(j[0]["seed"]["path"].as_str().unwrap(), "m/32'/133'/0'");

        assert!(j[1]["address"].as_str().unwrap().starts_with("t1"));
        let pk = j[1]["private_key"].as_str().unwrap();
        assert!(pk.starts_with("L") || pk.starts_with("K") || pk.starts_with("5"));

        // Check if all the addresses are the same
        let w = generate_wallet(true, false, 3, 3, &[]);
        let j = json::parse(&w).unwrap();
        assert_eq!(j.len(), 6);

        let mut set1 = HashSet::new();
        for i in 0..6 {
            set1.insert(j[i]["address"].as_str().unwrap());
            set1.insert(j[i]["private_key"].as_str().unwrap());
        }

        // There should be 6 + 6 distinct addresses and private keys
        assert_eq!(set1.len(), 12);
    }

    
    /// Test nohd address generation, which does not use the same sed.
    #[test]
    fn test_nohd() {
        use crate::paper::generate_wallet;
        use std::collections::HashSet;
        
        // Check if all the addresses use a different seed
        let w = generate_wallet(true, 3, 0, &[]);
        let j = json::parse(&w).unwrap();
        assert_eq!(j.len(), 3);

        let mut set1 = HashSet::new();
        let mut set2 = HashSet::new();
        for i in 0..3 {
            assert!(j[i]["address"].as_str().unwrap().starts_with("ztestsapling"));
            assert_eq!(j[i]["seed"]["path"].as_str().unwrap(), "m/32'/1'/0'");      // All of them should use the same path

            set1.insert(j[i]["address"].as_str().unwrap());
            set1.insert(j[i]["private_key"].as_str().unwrap());

            set2.insert(j[i]["seed"]["HDSeed"].as_str().unwrap());
        }

        // There should be 3 + 3 distinct addresses and private keys
        assert_eq!(set1.len(), 6);
        // ...and 3 different seeds
        assert_eq!(set2.len(), 3);
    }

    /// Test the address derivation against the test data (see below)
    fn test_address_derivation(testdata: &str) {
        use crate::paper::gen_addresses_with_seed_as_json;
        let td = json::parse(&testdata.replace("'", "\"")).unwrap();
        
        for i in td.members() {
            let seed = hex::decode(i["seed"].as_str().unwrap()).unwrap();
            let num  = i["num"].as_u32().unwrap();

            let addresses = gen_addresses_with_seed_as_json(num+1, 0, |child| (seed.clone(), child));

            let j = json::parse(&addresses).unwrap();
            assert_eq!(j[num as usize]["address"], i["addr"]);
            assert_eq!(j[num as usize]["private_key"], i["pk"]);
        }
    }

    #[test]
    fn test_vanity() {
        use crate::paper::generate_vanity_wallet;

        // Single thread
        let td = json::parse(&generate_vanity_wallet(false, 1, "te".to_string()).unwrap()).unwrap();
        assert_eq!(td.len(), 1);
        assert!(td[0]["address"].as_str().unwrap().starts_with("zs1te"));

        // Multi thread
        let td = json::parse(&generate_vanity_wallet(false, 4, "tt".to_string()).unwrap()).unwrap();
        assert_eq!(td.len(), 1);
        assert!(td[0]["address"].as_str().unwrap().starts_with("zs1tt"));

        // Testnet
        let td = json::parse(&generate_vanity_wallet(true, 4, "ts".to_string()).unwrap()).unwrap();
        assert_eq!(td.len(), 1);
        assert!(td[0]["address"].as_str().unwrap().starts_with("ztestsapling1ts"));

        // Test for invalid chars
        generate_vanity_wallet(false, 1, "b".to_string()).expect_err("b is not allowed");
        generate_vanity_wallet(false, 1, "o".to_string()).expect_err("o is not allowed");
        generate_vanity_wallet(false, 1, "i".to_string()).expect_err("i is not allowed");
        generate_vanity_wallet(false, 1, "1".to_string()).expect_err("1 is not allowed");
    }
}