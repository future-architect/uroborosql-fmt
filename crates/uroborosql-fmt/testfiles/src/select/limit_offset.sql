SELECT
    TBL1.COLUMN1    
    AS  COLUMN1
FROM
    TABLE1  TBL1
ORDER BY
    TBL1.COLUMN2    
    DESC
limit 5
OFFSET 5;

SELECT
    TBL1.COLUMN1    
    AS  COLUMN1
FROM
    TABLE1  TBL1
ORDER BY
    TBL1.COLUMN2    
    DESC
limit /*$hoge*/5
OFFSET 5;

SELECT
    TBL1.COLUMN1    
    AS  COLUMN1
FROM
    TABLE1  TBL1
ORDER BY
    TBL1.COLUMN2    
    DESC
limit 
all
OFFSET 5;

SELECT
    TBL1.COLUMN1    
    AS  COLUMN1
FROM
    TABLE1  TBL1
ORDER BY
    TBL1.COLUMN2    
    DESC
limit /*$hoge*/all
OFFSET 5;

SELECT
    TBL1.COLUMN1    
    AS  COLUMN1
FROM
    TABLE1  TBL1
ORDER BY
    TBL1.COLUMN2    
    DESC
limit 1 + 2
OFFSET 5;

SELECT
    TBL1.COLUMN1    
    AS  COLUMN1
FROM
    TABLE1  TBL1
ORDER BY
    TBL1.COLUMN2    
    DESC
limit /*$hoge*/100 + 1
OFFSET 5;
