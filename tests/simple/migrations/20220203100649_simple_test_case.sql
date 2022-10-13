-- Add migration script here
CREATE TABLE table_1(
   id INT NOT NULL,
   name VARCHAR(255) NOT NULL,
   PRIMARY KEY(id)
);

CREATE TABLE table_2(
   id INT NOT NULL,
   table_1_id INT,
   name VARCHAR(255) NOT NULL,
   PRIMARY KEY(id),
   CONSTRAINT fk_table_1
      FOREIGN KEY(table_1_id) 
	  REFERENCES table_1(id)
);

CREATE TABLE table_3(
   id INT NOT NULL,
   table_2_id INT,
   name VARCHAR(255) NOT NULL,
   PRIMARY KEY(id),
   CONSTRAINT fk_table_2
      FOREIGN KEY(table_2_id) 
	  REFERENCES table_2(id)
);

CREATE TABLE table_4(
   id INT NOT NULL,
   name VARCHAR(255) NOT NULL
);
