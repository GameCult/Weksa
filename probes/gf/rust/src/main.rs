//! Probe: load a PGF through the GF C runtime (libpgf/libgu) with hand-written
//! extern "C" declarations, linearize hand-written abstract trees into every
//! concrete language and every field of the lincat table, parse each string back,
//! and report parse counts and whether the original tree is among them.
//! No crate dependencies, no bindgen, no C shim.
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;

type Pool = c_void;
type Exn = c_void;
type Pgf = c_void;
type Concr = c_void;
type GuIn = c_void;
type GuOut = c_void;
type StrBuf = c_void;
type Enum = c_void;
type PgfType = c_void;
type Expr = usize; // GuVariant is uintptr_t
type CncTree = usize;

#[repr(C)]
struct ExprProb {
    prob: f32,
    expr: Expr,
}

#[link(name = "gu")]
extern "C" {
    fn gu_new_pool() -> *mut Pool;
    fn gu_pool_free(p: *mut Pool);
    fn gu_new_exn(p: *mut Pool) -> *mut Exn;
    fn gu_exn_is_raised(e: *mut Exn) -> bool;
    fn gu_string_in(s: *const c_char, p: *mut Pool) -> *mut GuIn;
    fn gu_new_string_buf(p: *mut Pool) -> *mut StrBuf;
    fn gu_string_buf_out(sb: *mut StrBuf) -> *mut GuOut;
    fn gu_out_flush(o: *mut GuOut, e: *mut Exn);
    fn gu_string_buf_freeze(sb: *mut StrBuf, p: *mut Pool) -> *const c_char;
    fn gu_enum_next(en: *mut Enum, to: *mut c_void, p: *mut Pool);
}

#[link(name = "pgf")]
extern "C" {
    fn pgf_read(path: *const c_char, p: *mut Pool, e: *mut Exn) -> *mut Pgf;
    fn pgf_get_language(pgf: *mut Pgf, lang: *const c_char) -> *mut Concr;
    fn pgf_read_expr(i: *mut GuIn, p: *mut Pool, tmp: *mut Pool, e: *mut Exn) -> Expr;
    fn pgf_read_type(i: *mut GuIn, p: *mut Pool, tmp: *mut Pool, e: *mut Exn) -> *mut PgfType;
    fn pgf_print_expr(x: Expr, ctx: *mut c_void, prec: c_int, o: *mut GuOut, e: *mut Exn);
    fn pgf_expr_eq(a: Expr, b: Expr) -> c_int;
    fn pgf_lzr_concretize(c: *mut Concr, x: Expr, e: *mut Exn, p: *mut Pool) -> *mut Enum;
    fn pgf_lzr_get_table(c: *mut Concr, t: CncTree, n: *mut usize, labels: *mut *const *const c_char);
    fn pgf_lzr_linearize_simple(c: *mut Concr, t: CncTree, idx: usize, o: *mut GuOut, e: *mut Exn, p: *mut Pool);
    fn pgf_parse(c: *mut Concr, ty: *mut PgfType, s: *const c_char, e: *mut Exn, p: *mut Pool, outp: *mut Pool) -> *mut Enum;
}

// GuExn starts { state: enum, caught: *const c_char, .. }; gu_exn_clear is static
// inline (state = GU_EXN_OK = 1, caught = NULL), so reproduce it here.
#[repr(C)]
struct ExnRepr {
    state: c_int,
    caught: *const c_char,
}
unsafe fn clear_exn(e: *mut Exn) {
    let r = e as *mut ExnRepr;
    (*r).state = 1;
    (*r).caught = ptr::null();
}

fn cs(s: &str) -> CString {
    CString::new(s).unwrap()
}
unsafe fn rs(p: *const c_char) -> String {
    CStr::from_ptr(p).to_string_lossy().into_owned()
}

unsafe fn print_expr(x: Expr, pool: *mut Pool, exn: *mut Exn) -> String {
    let sb = gu_new_string_buf(pool);
    let out = gu_string_buf_out(sb);
    pgf_print_expr(x, ptr::null_mut(), 1, out, exn);
    gu_out_flush(out, exn);
    rs(gu_string_buf_freeze(sb, pool))
}

unsafe fn read_expr(s: &str, pool: *mut Pool, exn: *mut Exn) -> Expr {
    let tmp = gu_new_pool();
    let cstr = cs(s); // gu_string_in does not copy: keep the buffer alive
    let i = gu_string_in(cstr.as_ptr(), tmp);
    let x = pgf_read_expr(i, pool, tmp, exn);
    gu_pool_free(tmp);
    x
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pgf_path = args.get(1).expect("usage: gfprobe <file.pgf> <Lang>...");
    let langs: Vec<&String> = args[2..].iter().collect();
    // Hand-built abstract trees (no model, no parser in the loop).
    let a = "UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))";
    let b = "UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N)))))";
    // (name, tree, parse category). Utt is the raw RGL table (all Style cells); Phr wraps one register cell.
    let owned: Vec<(String, String, &str)> = vec![
        ("a-Utt".into(), a.into(), "Utt"),
        ("a-Plain".into(), format!("PhrPlain ({a})"), "Phr"),
        ("a-Resp".into(), format!("PhrResp ({a})"), "Phr"),
        ("b-Utt".into(), b.into(), "Utt"),
        ("b-Plain".into(), format!("PhrPlain ({b})"), "Phr"),
        ("b-Resp".into(), format!("PhrResp ({b})"), "Phr"),
    ];
    unsafe {
        let pool = gu_new_pool();
        let exn = gu_new_exn(pool);
        let t0 = std::time::Instant::now();
        let pgf = pgf_read(cs(pgf_path).as_ptr(), pool, exn);
        assert!(!gu_exn_is_raised(exn) && !pgf.is_null(), "pgf_read failed");
        println!("pgf_read_ms\t{:.3}", t0.elapsed().as_secs_f64() * 1000.0);
        let read_type = |name: &str| {
            let tmp = gu_new_pool();
            let cstr = cs(name); // kept alive across the read
            let i = gu_string_in(cstr.as_ptr(), tmp);
            let t = pgf_read_type(i, pool, tmp, exn);
            gu_pool_free(tmp);
            t
        };
        println!("tree\tlang\tfield\tstring\tn_parses\toriginal_among\tparse_trees");
        for (tname, tsrc, pcat) in owned.iter() {
            let utt = read_type(pcat);
            let tname = tname.as_str();
            let tsrc = tsrc.as_str();
            let expr = read_expr(tsrc, pool, exn);
            assert!(!gu_exn_is_raised(exn), "bad tree {tname}");
            let round = print_expr(expr, pool, exn);
            println!("# tree {tname} read back as: {round}");
            for lang in &langs {
                let concr = pgf_get_language(pgf, cs(lang).as_ptr());
                assert!(!concr.is_null(), "no language {lang}");
                let tp = gu_new_pool();
                let en = pgf_lzr_concretize(concr, expr, exn, tp);
                assert!(!gu_exn_is_raised(exn), "concretize failed");
                let mut variant = 0;
                loop {
                    let mut ct: CncTree = 0;
                    gu_enum_next(en, &mut ct as *mut _ as *mut c_void, tp);
                    if ct == 0 {
                        break;
                    }
                    variant += 1;
                    let mut n = 0usize;
                    let mut labels: *const *const c_char = ptr::null();
                    pgf_lzr_get_table(concr, ct, &mut n, &mut labels);
                    for f in 0..n {
                        let label = rs(*labels.add(f));
                        let sp = gu_new_pool();
                        let sb = gu_new_string_buf(sp);
                        let o = gu_string_buf_out(sb);
                        pgf_lzr_linearize_simple(concr, ct, f, o, exn, sp);
                        gu_out_flush(o, exn);
                        let text = rs(gu_string_buf_freeze(sb, sp));
                        gu_pool_free(sp);
                        // parse it back in the same language
                        let pp = gu_new_pool();
                        let outp = gu_new_pool();
                        clear_exn(exn);
                        let ctext = cs(&text);
                        let pe = pgf_parse(concr, utt, ctext.as_ptr(), exn, pp, outp);
                        let mut n_parses = 0;
                        let mut among = false;
                        let mut shown = Vec::new();
                        if !pe.is_null() && !gu_exn_is_raised(exn) {
                            loop {
                                let mut ep: *mut ExprProb = ptr::null_mut();
                                gu_enum_next(pe, &mut ep as *mut _ as *mut c_void, pp);
                                if ep.is_null() {
                                    break;
                                }
                                n_parses += 1;
                                if pgf_expr_eq((*ep).expr, expr) != 0 {
                                    among = true;
                                }
                                if shown.len() < 4 {
                                    shown.push(print_expr((*ep).expr, pool, exn));
                                }
                            }
                        }
                        let failed = gu_exn_is_raised(exn);
                        clear_exn(exn);
                        println!(
                            "{tname}\t{lang}#v{variant}\t{label}\t{text}\t{n_parses}\t{among}\t{}{}",
                            if among && n_parses == 1 { String::from("(original only)") } else { shown.join(" | ") },
                            if failed { " [parse raised]" } else { "" }
                        );
                        gu_pool_free(pp);
                        gu_pool_free(outp);
                    }
                }
                gu_pool_free(tp);
            }
        }
        gu_pool_free(pool);
    }
}
