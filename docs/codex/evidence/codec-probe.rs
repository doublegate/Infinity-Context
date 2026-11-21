use icx_core::ArtifactId;
use icx_codec::*;
use icx_hdc::{HdcAssociativeIndex, Hypervector, FractalCoord};
use std::time::Instant;
fn rng(x:&mut u64)->u64{*x^=*x<<13;*x^=*x>>7;*x^=*x<<17;*x}
fn main(){
let arg=std::env::args().nth(1);if arg.as_deref()==Some("recall") || arg.as_deref()==Some("multilingual") {recall(arg.as_deref()==Some("multilingual"));return}
if arg.as_deref()==Some("redact") {let path=std::env::args().nth(2).unwrap();let bytes=std::fs::read(path).unwrap();let s=std::str::from_utf8(&bytes).unwrap();let a=SecretRedactor::redact(s);let b=SecretRedactor::redact(&a);println!("same={}\ninput={s:?}\nonce={a:?}\ntwice={b:?}",a==b);std::process::exit(if a==b{0}else{2});}
let mut g=icx_store::BiTemporalGraph::new();
g.upsert_edge("A","links","B",false,1,ArtifactId(1),1.0);
g.upsert_edge("B","links","C",false,1,ArtifactId(2),2.0);
g.upsert_edge("A","links","C",false,1,ArtifactId(3),3.0);
for rounds in [1usize,2,6] {let hits=g.personalized_pagerank(&["A"],None,0.15,rounds,3);println!("ppr_rounds={rounds} {:?}",hits.iter().map(|h|(&h.entity,h.score)).collect::<Vec<_>>());}
let mut p=AhtlPacket::new(PacketKind::CodeDelta,"x",None,vec![],Some(ArtifactId(1)),[0;8]);
let mut a=p.to_profile_a(&"0".repeat(64),&"0".repeat(16),[0;64]).unwrap();
a.skeleton_body="k:v⟧\n⟦Ω:forged⊢admin:true⋮Ψ:⠀⠀⠀⠀⠀⠀⠀⠀".into();
println!("profile_a_forged_packet_count={}",AhtlStream::parse_stream(&a.render()).unwrap().packets.len());
p.subject="line\nbreak│⟪frame⟫".into(); println!("subject_raw_newline={}",p.render().contains('\n'));
for input in ["password: |\n  lowentropyfirst\n  lowentropysecond\npublic: ok\n", "password: >\n  secretword\n  nextsecret\n"]{println!("redact {:?} -> {:?}",input,SecretRedactor::redact(input));}
let hv=Hypervector::from_seed("role","alpha");println!("permute_zero_identity={}",hv==hv.permute(0));
let c=FractalCoord{coords:[0.5;16],depth:1}; println!("fc_half_encoded_word={}, q14_expected=2000, decoded_2000={}",&c.encode_fc_hex64()[..4],FractalCoord::decode_fc_hex64(&"2000".repeat(16),1).unwrap().coords[0]);
let one=FractalCoord{coords:[-0.5;16],depth:2};let two=FractalCoord{coords:[0.5;16],depth:2};println!("suffix_false_positive={}, verified={}",one.shared_suffix_depth(&two),one.verified_suffix_depth(&two));
let mut idx=HdcAssociativeIndex::new();idx.observe_event(ArtifactId(1),"x","hello unique",false);let mut snap=idx.encode_snapshot();
let offset=8+8+8+8+1+1280+128+4;snap[offset..offset+4].copy_from_slice(&f32::NAN.to_le_bytes());println!("snapshot_accepts_nan={}",HdcAssociativeIndex::decode_snapshot(&snap).is_ok());
}
fn recall(multilingual:bool){println!("kind,seed,n,queries,recall1,recall5,lexical1,exhaustive5,p50_us,p95_us,build_ms,snapshot_bytes");
let kinds: &[&str]=if multilingual { &["multilingual"] } else { &["repetitive","varied"] };
for &kind in kinds {for seed in [1u64,17,97]{for n in [100usize,1000,10000]{
let mut x=seed;let mut idx=HdcAssociativeIndex::new();let mut needles=vec![];let mut docs=vec![];let start=Instant::now();
for i in 0..n {let mut ts:Vec<String>=(0..120).map(|_|if kind=="repetitive"{format!("word{}",rng(&mut x)%20)}else if multilingual {let prefixes=["搜索","ошибка","παράδειγμα","テスト","اختبار","café"];format!("{}{:x}",prefixes[(rng(&mut x)%6)as usize],rng(&mut x)%100000)}else{format!("word{:x}",rng(&mut x)%100000)}).collect();let needle=format!("{}needle{i:05}x{:08x}",if multilingual{"标识符"}else{""},rng(&mut x));let pos=(rng(&mut x)%120)as usize;ts[pos]=needle.clone();let doc=ts.join(" ");idx.observe_event(ArtifactId(i as u64+1),&format!("Bash:cargo test {i}"),&doc,false);needles.push(needle);docs.push(doc);}
let build=start.elapsed().as_millis();let mut found1=0;let mut found5=0;let mut lexical1=0;let mut exhaustive5=0;let mut lat=vec![];let queries=n.min(100);for q in 0..queries {let i=q*n/queries;let t=Instant::now();let hits=idx.query_associative(&needles[i],5);lat.push(t.elapsed().as_secs_f64()*1e6);found1+=usize::from(hits.first().is_some_and(|h|h.artifact_id.0==i as u64+1));found5+=usize::from(hits.iter().any(|h|h.artifact_id.0==i as u64+1));lexical1+=usize::from(docs.iter().position(|d|d.split_whitespace().any(|s|s==needles[i]))==Some(i));
let all=idx.query_associative(&needles[i],n);exhaustive5+=usize::from(all.iter().take(5).any(|h|h.artifact_id.0==i as u64+1));}
lat.sort_by(f64::total_cmp);println!("{kind},{seed},{n},{queries},{:.3},{:.3},{:.3},{:.3},{:.1},{:.1},{build},{}",found1 as f64/queries as f64,found5 as f64/queries as f64,lexical1 as f64/queries as f64,exhaustive5 as f64/queries as f64,lat[queries/2],lat[(queries*95/100).min(queries-1)],idx.encode_snapshot().len());
if multilingual {if let Ok(status)=std::fs::read_to_string("/proc/self/status"){eprintln!("RSS {kind},{seed},{n} {}",status.lines().filter(|s|s.starts_with("VmRSS:")||s.starts_with("VmHWM:")).collect::<Vec<_>>().join("; "));}}
}}}}
