import time
from src.window_tracker import window_name

def main():
    while True:
        window_name()
        time.sleep(2)


if __name__ == "__main__":
    main()