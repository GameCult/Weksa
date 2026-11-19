interface LexProbe = open Syntax in {
  oper
    good_N, water_N, way_N : N ;
    high_A, constant_A : A ;
    like_A2 : A2 ;
    speak_V2 : V2 ;
    plainStr, respStr : Utt -> Str ;   -- pick a register cell out of an Utt
}
