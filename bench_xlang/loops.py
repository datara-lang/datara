acc = 0
i = 0
while i < 10000000:
    acc += i & 255
    i += 1
