-- Basic seach query
SELECT id, description, rating, category FROM search_config.search('category:electronics');
-- With limit
SELECT id, description, rating, category FROM search_config.search('category:electronics', limit_rows => 2);
-- With limit and offset
SELECT id, description, rating, category FROM search_config.search('category:electronics', limit_rows => 2, offset_rows => 1);
-- With fuzzy field
SELECT id, description, rating, category FROM search_config.search('category:electornics', fuzzy_fields => 'category');
-- Without fuzzy field
SELECT id, description, rating, category FROM search_config.search('category:electornics');
-- With fuzzy field and transpose_cost_one=false and distance=1
SELECT id, description, rating, category FROM search_config.search('description:keybaord', fuzzy_fields => 'description', transpose_cost_one => false, distance => 1);
-- With fuzzy field and transpose_cost_one=true and distance=1
SELECT id, description, rating, category FROM search_config.search('description:keybaord', fuzzy_fields => 'description', transpose_cost_one => true, distance => 1);
-- With regex field 
SELECT id, description, rating, category FROM search_config.search('com', regex_fields => 'description');
-- Default highlighting without max_num_chars
SELECT description, rating, category, highlight_bm25 FROM search_config.search('description:keyboard OR category:electronics') as s LEFT JOIN search_config.highlight('description:keyboard OR category:electronics', highlight_field => 'description') as h ON s.id = H.id LEFT JOIN search_config.rank('description:keyboard OR category:electronics') as r ON s.id = r.id ORDER BY rank_bm25 DESC LIMIT 5;
-- max_num_chars is set to 14 
SELECT description, rating, category, highlight_bm25 FROM search_config.search('description:keyboard OR category:electronics', max_num_chars => 14) as s LEFT JOIN search_config.highlight('description:keyboard OR category:electronics', highlight_field => 'description', max_num_chars => 14) as h ON s.id = H.id LEFT JOIN search_config.rank('description:keyboard OR category:electronics', max_num_chars => 14) as r ON s.id = r.id ORDER BY rank_bm25 DESC LIMIT 5;
--- With fuzzy field prefix disabled
SELECT id, description, category, rating FROM search_config.search('key', fuzzy_fields => 'description,category', distance => 2, transpose_cost_one => false, prefix => false, limit_rows => 5);
--- With fuzzy field prefix enabled
SELECT id, description, category, rating FROM search_config.search('key', fuzzy_fields => 'description,category', distance => 2, transpose_cost_one => false, prefix => true, limit_rows => 5);

