# CultCache Lowering Examples

These examples lower interlingua packets for separate target languages. The
language ontology that owns en-US and ja-JP choices is `data/ontology/ontology.cc`
(`weksa ontology dump`); its entries are model-drafted seeds.

Each target file is produced as a separate agent run so differences in target
ontology remain inspectable instead of being smoothed into one multilingual
batch.
