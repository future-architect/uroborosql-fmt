-- testfiles/src/select/asterisk.sql
select tab.* -- asterisk
, tab2.hoge AS hoge -- hoge
from tab, tab2
;
