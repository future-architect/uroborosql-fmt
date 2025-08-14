DELETE FROM products
  WHERE obsoletion_date = 'today'
  RETURNING *;
DELETE FROM products
  WHERE obsoletion_date = 'today'
  RETURNING -- comment
   *;
