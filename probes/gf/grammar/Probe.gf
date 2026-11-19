abstract Probe = {
  flags startcat = Phr ;
  cat Phr ; Utt ; S ; Cl ; NP ; CN ; AP ; RS ; N ; A ; A2 ; V2 ;
  fun
    PhrPlain, PhrResp : Utt -> Phr ;      -- register is a tree-level choice
    UttS    : S -> Utt ;
    PosS    : Cl -> S ;
    NegS    : Cl -> S ;
    ClNPNP  : NP -> NP -> Cl ;            -- x is y
    ClNPAP  : NP -> AP -> Cl ;            -- x is <adjectival phrase>
    TheCN   : CN -> NP ;
    BareN   : N -> NP ;                   -- mass noun, no determiner
    UseN    : N -> CN ;
    ModCN   : AP -> CN -> CN ;
    RelCN   : CN -> RS -> CN ;
    SuperlA : A -> AP ;                   -- the highest
    PosA    : A -> AP ;
    LikeAP  : A2 -> NP -> AP ;            -- like water
    CanBePassedRS : V2 -> RS ;            -- that can be <V2-ed>
    -- lexicon
    good_N, water_N, way_N : N ;
    high_A, constant_A : A ;
    like_A2 : A2 ;
    speak_V2 : V2 ;
}
