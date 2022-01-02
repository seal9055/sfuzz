#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <stddef.h> 
#include <stdint.h>
#include <unistd.h>
#include <fcntl.h>

//gcc zip_parser.c -o zip_parser -fno-stack-protector -no-pie

struct head {
	uint16_t num_files;
	uint32_t centdir_size;
	uint32_t centdir_offset;
};

struct centdir {
	uint32_t comp_size;
	uint32_t filerecord_offset;
	uint16_t filename_len;
	uint16_t extrafield_len;
};

struct dataentry {
	char *comp_data;
	uint32_t comp_size;
	uint16_t extrafield_len;
};

struct head *header = NULL;
struct centdir *entrylist = NULL;
struct dataentry *data = NULL;

void print_info() {
	printf("\nHeader:\n");
	printf("\tnum_files: %d\n", header->num_files);
	printf("\tcentdir_size: %d\n", header->centdir_size);
	printf("\tcentdir_offset: %d\n", header->centdir_offset);

	printf("Centdir:\n");
	printf("\tcomp_size: %d\n", entrylist->comp_size);
	printf("\tfilerecord_offset: %d\n", entrylist->filerecord_offset);
	printf("\tfilename_len: %d\n", entrylist->filename_len);
	printf("\textrafield_len: %d\n", entrylist->extrafield_len);

	for (int i = 0; i < header->num_files; i++) {
		printf("Data:\n");
		printf("\tcomp_size: %d\n", data[i].comp_size);
		printf("\textrafield_len: %d\n", data[i].extrafield_len);
		printf("\tuncompressed_data: %s\n", data[i].comp_data);
	}
}


void parse_data(char *buffer, int i) {
	char buf[128];
	int offset, len;

	for (i = 0; i < header->num_files; i++) {

		memcpy(&data[i].extrafield_len, &buffer[entrylist[i].filerecord_offset + 28], sizeof(short));
		memcpy(&data[i].comp_size, &buffer[entrylist[i].filerecord_offset + 18], sizeof(int));
		offset = 30 + entrylist[i].filerecord_offset + entrylist[i].filename_len + data[i].extrafield_len;

		len = data[i].comp_size;
		data[i].comp_data = malloc(len);

		memcpy(buf, &buffer[offset], len);
		buf[128] = '\0';
		strcpy(data[i].comp_data, buf);
	}
}

void parse_centdir(char *buffer) {
	int offset = 0;

	for (int i = 0; i < header->num_files; i++) {
		memcpy(&entrylist[i].comp_size, &buffer[offset + header->centdir_offset + 20], sizeof(int));
		memcpy(&entrylist[i].filerecord_offset, &buffer[offset + header->centdir_offset + 42], sizeof(int));

		memcpy(&entrylist[i].filename_len, &buffer[offset + header->centdir_offset + 28], sizeof(short));
		memcpy(&entrylist[i].extrafield_len, &buffer[offset + header->centdir_offset + 30], sizeof(short));

		if (entrylist[i].comp_size > 128) {
			exit(1);
		}
		offset += 46 + entrylist[i].extrafield_len + entrylist[i].filename_len;
	}
}

int parse_head(char *buffer, int len) {
	char magic[] = {0x50, 0x4B, 0x05, 0x06};
	int header_offset;

	for (int i = 0; i < len-4; i++) {
		if (memcmp(&buffer[i], magic, 4) == 0) {
			header_offset = i;
			memcpy(&header->num_files, &buffer[header_offset+10], sizeof(short));
			memcpy(&header->centdir_size, &buffer[header_offset+12], sizeof(int));
			memcpy(&header->centdir_offset, &buffer[header_offset+16], sizeof(int));
			return 0;
		}
	}
	return 1;
}

int main(int argc, char **argv) {
	char *buffer;
	char file_name[32];
	int fd, size;

	if (argc != 2) {
		puts("Please provide file to parse");
		exit(1);
	}

	strncpy(file_name, argv[1], 30);
	file_name[strcspn(file_name, "\n")] = 0;

	// Open file
	if ((fd = open(file_name, O_RDONLY, 0)) <= 0) {
		puts("Failed to open file");
		exit(1);
	}

	// Get file size
	size = lseek(fd, 0, SEEK_END);
	lseek(fd, 0, SEEK_SET);


	buffer = (char *) malloc(size);
	read(fd, buffer, size);

	header = malloc(sizeof(struct head));

	if (parse_head(buffer,size)) {
		puts("Failed to parse header");
		exit(1);
	}
	
	entrylist = malloc(sizeof(struct centdir) * header->num_files);
	parse_centdir(buffer);

	data = malloc(sizeof(struct dataentry) * header->num_files);
	parse_data(buffer, 0);

	print_info();
}
