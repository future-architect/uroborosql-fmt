SELECT
    *
FROM multilingual_test
ORDER BY
japanese_text
     COLLATE 
     /*$LC_COLLATE*/"ja_JP.UTF-8"        DESC,
    german_text COLLATE          "de_DE.UTF-8";