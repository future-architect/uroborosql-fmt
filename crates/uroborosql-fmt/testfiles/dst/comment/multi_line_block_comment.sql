/*
	通常のコメント
*/
select
	something	as	something
from
	somewhere
/*
    行頭に
        空白を持つコメント
*/
select
	something	as	something
from
	somewhere
/*
    *    コメント
    コメント
    *    コメント
*/
select
	something	as	something
from
	somewhere
-- aligned with asterisk
/*
 * コメント
 * コメント
 */
select
	something	as	something
from
	somewhere
/*
 * そろっていない
 * コメント
 */
select
	something	as	something
from
	somewhere
/*
 * コメント
 * コメント
 */
select
	something	as	something
from
	somewhere
select
	something	as	something
/*
 *        コメント
 *        コメント
 */
from
	somewhere
-- nested
select
	*
from
	(
		/*
		 * コメント
		 * コメント
		 */
		select
			*
		from
			foo	f
	)
select
	*
from
	(
		/*
		 * コメント
		 * コメント
		 */
		select
			*
		from
			foo	f
	)
select
	*
from
	(
		/*
		 * コメント
		 * コメント
		 * コメント
		 */
		select
			*
		from
			foo	f
	)
