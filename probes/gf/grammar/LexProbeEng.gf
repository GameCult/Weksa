instance LexProbeEng of LexProbe = open SyntaxEng, ParadigmsEng in {
  oper
    good_N = mkN "good" ;
    water_N = mkN "water" ;
    way_N = mkN "way" ;
    high_A = mkA "high" ;
    constant_A = mkA "constant" ;
    like_A2 = mkA2 (mkA "like") noPrep ;
    speak_V2 = mkV2 (mkV "speak" "spoke" "spoken") ;
    plainStr = \u -> u.s ;
    respStr = \u -> u.s ;
}
