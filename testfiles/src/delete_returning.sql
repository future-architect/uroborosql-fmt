DELETE FROM products
  WHERE obsoletion_date = 'today'
  RETURNING *;
