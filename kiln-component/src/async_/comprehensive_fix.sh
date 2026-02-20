#\!/bin/bash

# Comprehensive fix for all corrupted error messages in async files
for file in /Users/r/git/wrt2/wrt-component/src/async_/*.rs; do
    if [ -f "$file" ]; then
        echo "Fixing $(basename $file)..."
        
        # Fix the main corrupted pattern: "Error occurred"MessageMissing message" -> "Message"
        perl -i -pe 's/"Error occurred"([^"]*?)(?:Missing message)+/"$1"/g' "$file"
        
        # Fix standalone Missing message patterns
        perl -i -pe 's/Missing message(?:\)|,|;|")/ "Error message"$1/g' "$file"
        
        # Fix format\! patterns
        perl -i -pe 's/format\!\(Missing message\),/format\!("Error message"),/g' "$file"
        
        # Fix log patterns
        perl -i -pe 's/log::(warn|info|error)\!\(Missing message\);/log::$1\!("Log message");/g' "$file"
        
        # Remove any leftover quotes issues
        perl -i -pe 's/""([^"]*)/"$1"/g' "$file"
        
        # Fix missing closing parentheses after Error::
        perl -i -pe 's/(Error::\w+\([^)]*)"(\)|\);)/$1")$2/g' "$file"
        
        echo "Fixed $(basename $file)"
    fi
done

echo "Comprehensive fix completed"
