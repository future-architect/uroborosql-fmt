/*
comment
*/
select 1

/*
        comment
            comment with space
*/
select 1

/*
    *    will 
    not 
    *   aligned 
*/
select 1

-- aligned with asterisk
/*
 * a
 * b
 */
select 1

/*
* a
* b
*/
select 1

/*
    *a
*b
*/
select 1

    /*
    *        a
    *        b
    */
select 1 

/*
*a
* b
   *  c */
select 1

-- nested
select *
from (
    /*
     * a
     * b
     */
    select 
        *
    from
        foo f
)

select *
from (
/*
    * a
* b
    */
    select 
        *
    from
        foo f
)

select *
from (
                /*
        * a
            *b
        *  c
    */
    select 
        *
    from
        foo f
)
