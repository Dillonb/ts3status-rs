import sqlite3
import sys


def migrate_data(db_path):
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    try:
        # Read all data from the parsed_client table
        cursor.execute("SELECT unique_id, nickname, last_seen FROM parsed_client")
        parsed_clients = cursor.fetchall()

        # Prepare data for insertion into user_cache
        user_cache_data = []
        for client in parsed_clients:
            unique_id, nickname, last_seen = client

            if last_seen is None:
                print(f"Skipping entry for {nickname} due to NULL timestamp.")
                continue

            user_cache_data.append((unique_id, nickname, last_seen))

        # Insert data into the user_cache table
        cursor.executemany(
            "INSERT INTO user_cache (unique_id, nickname, last_seen_timestamp) VALUES (?, ?, ?)",
            user_cache_data,
        )

        conn.commit()
        print(f"Successfully migrated {len(user_cache_data)} records.")

    except sqlite3.Error as e:
        print(f"An error occurred: {e}")
        conn.rollback()
    finally:
        conn.close()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python migrate_db.py <database_file>")
        sys.exit(1)

    db_file = sys.argv[1]
    migrate_data(db_file)
