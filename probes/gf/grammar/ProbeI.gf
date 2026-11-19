incomplete concrete ProbeI of Probe = open Syntax, LexProbe in {
  lincat
    Phr = { s : Str } ;
    Utt = Syntax.Utt ; S = Syntax.S ; Cl = Syntax.Cl ; NP = Syntax.NP ; CN = Syntax.CN ;
    AP = Syntax.AP ; RS = Syntax.RS ; N = Syntax.N ; A = Syntax.A ; A2 = Syntax.A2 ; V2 = Syntax.V2 ;
  lin
    PhrPlain u = { s = plainStr u } ;
    PhrResp u = { s = respStr u } ;
    UttS s = mkUtt s ;
    PosS cl = mkS cl ;
    NegS cl = mkS negativePol cl ;
    ClNPNP a b = mkCl a b ;
    ClNPAP a b = mkCl a b ;
    TheCN cn = mkNP the_Det cn ;
    BareN n = mkNP n ;
    UseN n = mkCN n ;
    ModCN ap cn = mkCN ap cn ;
    RelCN cn rs = mkCN cn rs ;
    SuperlA a = mkAP (mkOrd a) ;
    PosA a = mkAP a ;
    LikeAP a np = mkAP a np ;
    CanBePassedRS v = mkRS (mkRCl which_RP (mkVP can_VV (passiveVP v))) ;
    good_N = LexProbe.good_N ;
    water_N = LexProbe.water_N ;
    way_N = LexProbe.way_N ;
    high_A = LexProbe.high_A ;
    constant_A = LexProbe.constant_A ;
    like_A2 = LexProbe.like_A2 ;
    speak_V2 = LexProbe.speak_V2 ;
}
