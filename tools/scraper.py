#!/usr/bin/python3
import requests
import urllib
from requests_html import HTML
from requests_html import HTMLSession
import os
import sys
import random
import string
import subprocess
from subprocess import DEVNULL
import signal
import time
from datetime import timedelta
import threading

'''
Web scrapper that makes use of google's search engine to collect files of a specific filetype. Let
it run as long as you wish, and ctrl-c to stop execution. A signal handler will then remove all
incorrectly downloaded files and dedup the collection to create a unique set of files given a
specific type.

Just modify FILE_TYPE to specify what type of file you want. Only filetypes that are indexable by
googles `filetype=XXX` are supported.
'''

# File type to collect
FILE_TYPE = "pdf"

# Delay between google-requests, can help reduce throttling (in seconds). 0-30 seconds seem to be
# best in most cases, the optimal delay varies
REQUEST_DELAY = 0
 
# Number of urls containing pdf files for which download attempts were made
num_downloads = 0

# Number of downloads that were attempted but failed
failed_downloads = 0

# Time that has elapsed since program start
elapsed = 0

# Get the source code of a requested page
def get_source(url):
    session = HTMLSession()
    response = session.get(url)
    return response

# Return google search results for a query
def scrape_google(query):
    query = urllib.parse.quote_plus(query)
    response = get_source("https://www.google.co.uk/search?q=" + query)

    links = list(response.html.absolute_links)
    google_domains = ('https://www.google.', 
                      'https://google.', 
                      'https://webcache.googleusercontent.', 
                      'http://webcache.googleusercontent.', 
                      'https://policies.google.',
                      'https://support.google.',
                      'https://maps.google.')

    # Remove all irrelevant links
    for url in links[:]:
        if url.startswith(google_domains):
            links.remove(url)
    return links

# Download requested url
def download_file(file_name, url):
    response = requests.get(url)
    open(file_name, "wb").write(response.content)

# Remove all files that were mistakenly downloaded and don't have the correct type
def remove_duds():
    ret = 0
    for file_name in os.listdir("seeds"):
        output = str(subprocess.check_output(f"file seeds/{file_name}", shell=True, stderr=DEVNULL))
        # If the file we downloaded has the incorrect file-type, remove
        if FILE_TYPE not in output and FILE_TYPE.upper() not in output:
            os.remove(f"seeds/{file_name}")
            ret += 1
    return ret

# Remove all duplicates in the downloaded files
def dedup():
    ret = 0
    unique = []
    for file_name in os.listdir("seeds"):
        if os.path.isfile(file_name):
            filehash = md5.md5(file(file_name).read()).hexdigest()
            if filehash not in unique: 
                unique.append(filehash)
            else: 
                os.remove(file_name)
                ret += 1
    return ret

# Print out the overall results
def print_results(duds_removed, deduped):
    global elapsed
    global num_downloads
    global failed_downloads

    num_files = len([name for name in os.listdir('seeds')])
    print("\n\n\n+===================================================+")
    print(f"Runtime: {str(timedelta(seconds=elapsed))}")
    print(f"Total initial download attempts: {num_downloads}")
    print(f"Failed downloads: {failed_downloads}")
    print(f"Incorrect file-types removed: {duds_removed}")
    print(f"Duplicate files removed: {deduped}")
    print(f"A total of {num_files} unique files now exist in the `seeds` directory")
    print("+===================================================+\n\n")

# Signal handler to print out invoke some filtering functions on the seed collections and print out
# results
def signal_handler(sig, frame):
    duds_removed = remove_duds()
    deduped = dedup()

    print_results(duds_removed, deduped)
    os.kill(os.getpid(), signal.SIGQUIT)

# Print out how much time has passed every second
def timer():
    global elapsed
    while True:
        elapsed += 1
        sys.stdout.write("\r")
        sys.stdout.write("Runtime: " + str(timedelta(seconds=elapsed)))
        time.sleep(1)

def main():
    global num_downloads
    global failed_downloads
    global elapsed

    # Start timer thread
    threading.Thread(target=timer).start()

    # Create seed directory if it doesnt already exist
    os.makedirs("seeds", exist_ok=True)

    while True:
        # Collect a list of urls that contain pdf files
        rand_search = ''.join(random.choice(string.ascii_lowercase) for i in range(10))
        query = f"filetype:{FILE_TYPE} {rand_search}"
        try:
            urls = scrape_google(query)
            # Download all previously found files
            num_downloads += len(urls)
            for url in urls:
                rand_name = ''.join(random.choice(string.ascii_lowercase) for i in range(10))
                try:
                    download_file(f"seeds/{rand_name}", url)
                except:
                    failed_downloads += 1
        except:
            pass

        time.sleep(REQUEST_DELAY)

if __name__ == "__main__":
    signal.signal(signal.SIGINT, signal_handler)
    print("Hit CTRL-C to stop execution at any time")
    main()
