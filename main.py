import time
from src.tracker import window_name
from src.db import db_path, db_initialize

def main():
    # while True:
    #     window_name()
    #     time.sleep(2)
    db_path()
    db_initialize()

if __name__ == "__main__":
    main()