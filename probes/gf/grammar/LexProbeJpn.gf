instance LexProbeJpn of LexProbe = open SyntaxJpn, ParadigmsJpn, ResJpn in {
  oper
    good_N = mkN "善" ;
    water_N = mkN "水" ;
    way_N = mkN "道" ;
    high_A = mkA "高い" ;
    constant_A = mkA "常の" ;
    like_A2 = mkA2 "ようだ" "の" ;
    speak_V2 = mkV2 "言う" ;
    plainStr = \u -> u.s ! Wa ! Plain ;
    respStr = \u -> u.s ! Wa ! Resp ;
}
