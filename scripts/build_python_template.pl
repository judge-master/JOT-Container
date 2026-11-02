#!/usr/bin/perl
# Download and build CPython, then generate a filesystem-free WASI template.
use strict;
use warnings;
use Cwd qw(abs_path);
use File::Basename qw(dirname);
use File::Path qw(make_path);
use File::Temp qw(tempdir);
use Getopt::Long qw(GetOptions);

$| = 1;
my %args;
GetOptions(\%args, 'source-dir=s', 'build-dir=s', 'native-build-dir=s',
    'wasi-sdk=s', 'cc=s', 'sysroot=s', 'output=s', 'python-version=s',
    'work-dir=s', 'download-dir=s', 'log-dir=s', 'jobs=i', 'verbose!') or die "Invalid arguments\n";
for (qw(output)) {
    die "--$_ is required\n" unless defined $args{$_};
}
die "Use either --wasi-sdk or --cc with --sysroot\n"
    unless ($args{'wasi-sdk'} && !$args{cc}) ||
           ($args{cc} && $args{sysroot} && !$args{'wasi-sdk'});
die "Unexpected arguments: @ARGV\n" if @ARGV;

sub read_file {
    my ($path) = @_;
    open my $fh, '<:raw', $path or die "$path: $!\n";
    local $/;
    return <$fh>;
}
sub run {
    system { $_[0] } @_;
    die "Command failed (@_): status $?\n" if $? != 0;
}
# Run noisy tools in a child so compiler diagnostics remain available in logs.
sub stage {
    my ($name, $dir, $env, @command) = @_;
    my $log = "$args{'log-dir'}/$name.log";
    print "[python-template] $name\n";
    my $pid = fork();
    die "Cannot fork: $!\n" unless defined $pid;
    if (!$pid) {
        chdir $dir or die "$dir: $!\n";
        open STDOUT, '>:raw', $log or die "$log: $!\n";
        open STDERR, '>&', \*STDOUT or die $!;
        @ENV{keys %$env} = values %$env;
        exec { $command[0] } @command;
        die "Cannot execute $command[0]: $!\n";
    }
    waitpid($pid, 0);
    my $status = $?;
    if ($status || $args{verbose}) {
        my @lines = split /(?<=\n)/, read_file($log);
        @lines = @lines[-60 .. -1] if !$args{verbose} && @lines > 60;
        print STDERR @lines;
    }
    die "Stage $name failed (status $status); full log: $log\n" if $status;
}

sub build_python {
    my $version = $args{'python-version'};
    die "--python-version must be a CPython 3.11+ release (for example 3.14.7)\n"
        unless $version =~ /^3\.(\d+)\.\d+$/ && $1 >= 11;
    die "--python-version cannot be combined with prebuilt source/build directories\n"
        if grep { defined $args{$_} } qw(source-dir build-dir native-build-dir);
    my $root = ($args{'work-dir'} // '/build/python') . "/$version";
    make_path($root);
    $root = abs_path($root);
    my $source = "$root/source";
    my $native = "$root/native";
    my $wasi = "$root/wasi";
    $args{'log-dir'} //= "$root/logs";
    make_path($args{'log-dir'}, $native, $wasi);
    $args{'log-dir'} = abs_path($args{'log-dir'});
    my $downloads = $args{'download-dir'} // "$root/downloads";
    make_path($downloads);
    $downloads = abs_path($downloads);
    my $archive = "$downloads/Python-$version.tar.xz";
    if (!-f "$source/Include/patchlevel.h") {
        if (!-f $archive) {
            stage('download', $root, {}, 'wget', '--no-verbose', '--retry-connrefused', '--tries=3',
                "https://www.python.org/ftp/python/$version/Python-$version.tar.xz", '-O', "$archive.part");
            rename "$archive.part", $archive or die $!;
        }
        my $unpack = tempdir('source-XXXXXX', DIR => $root, CLEANUP => 1);
        stage('extract', $root, {}, 'tar', '-xJf', $archive, '-C', $unpack, '--strip-components=1');
        rename $unpack, $source or die "Cannot install source: $!\n";
    }
    my $header = read_file("$source/Include/patchlevel.h");
    die "Source version does not match $version\n" unless $header =~ /^#define\s+PY_VERSION\s+"\Q$version\E"/m;
    my $jobs = $args{jobs};
    if (!defined $jobs) {
        open my $cpus, '-|', 'getconf', '_NPROCESSORS_ONLN' or die $!;
        $jobs = <$cpus>;
        close $cpus or die "Cannot detect CPU count; pass --jobs\n";
        chomp $jobs;
    }
    die "--jobs must be a positive integer\n" unless $jobs =~ /^[1-9]\d*$/;
    my $cc = abs_path($args{cc} // "$args{'wasi-sdk'}/bin/clang") // die "Missing WASI compiler\n";
    my $sysroot = abs_path($args{sysroot} // "$args{'wasi-sdk'}/share/wasi-sysroot") // die "Missing WASI sysroot\n";
    # Native Python is required by CPython's cross-build tooling.
    stage('configure-native', $native, { CFLAGS => '-O1' }, "$source/configure",
        '--without-ensurepip', '--disable-test-modules');
    stage('build-native', $native, {}, 'make', '-s', '-j', $jobs);
    my ($config_site) = grep { -f $_ } (
        "$source/Tools/wasm/wasi/config.site-wasm32-wasi",
        "$source/Tools/wasm/config.site-wasm32-wasi");
    die "Missing CPython WASI config.site\n" unless defined $config_site;
    open my $guess, '-|', "$source/config.guess" or die $!;
    my $host = <$guess>;
    close $guess or die "config.guess failed\n";
    chomp $host;
    # Older CPython HACL headers repeatedly redefine WASI SDK endian macros.
    my %env = (CC => "$cc --target=wasm32-wasip1 --sysroot=$sysroot",
        CFLAGS => '-O3 -Wno-macro-redefined',
        AR => dirname($cc) . '/llvm-ar', RANLIB => dirname($cc) . '/llvm-ranlib',
        CONFIG_SITE => $config_site, PKG_CONFIG_PATH => '',
        PKG_CONFIG_LIBDIR => "$sysroot/lib/pkgconfig", PKG_CONFIG_SYSROOT_DIR => $sysroot);
    stage('configure-wasi', $wasi, \%env, "$source/configure", '--host=wasm32-wasip1',
        "--build=$host", "--with-build-python=$native/python",
        '--without-ensurepip', '--disable-test-modules', '--disable-shared');
    stage('build-wasi', $wasi, {}, 'make', '-s', '-j', $jobs, 'python.wasm');
    @args{qw(source-dir build-dir native-build-dir)} = ($source, $wasi, $native);
}
if (defined $args{'python-version'}) { build_python(); }
for (qw(source-dir build-dir native-build-dir)) {
    die "--$_ or --python-version is required\n" unless defined $args{$_};
}
$args{'log-dir'} //= dirname($args{output}) . '/python-template-logs';
make_path($args{'log-dir'});
$args{'log-dir'} = abs_path($args{'log-dir'});
my $source = abs_path($args{'source-dir'}) // die "Missing source directory\n";
my $build = abs_path($args{'build-dir'}) // die "Missing WASI build directory\n";
my $native = abs_path($args{'native-build-dir'}) // die "Missing native build directory\n";
make_path(dirname($args{output}));
my $output = abs_path(dirname($args{output})) . '/' . (split m{/}, $args{output})[-1];
my $work = tempdir('jot-python-XXXXXX', TMPDIR => 1, CLEANUP => 1);

# Ask make to expand VERSION, ABIFLAGS and LIBRARY rather than guessing filenames.
open my $query, '>', "$work/config.mk" or die $!;
print {$query} ".PHONY: jot-template-config\njot-template-config:\n\t\@printf '%s\\n' '\$(VERSION)' '\$(LIBRARY)' '\$(srcdir)'\n";
close $query or die $!;
sub build_config {
    my ($dir) = @_;
    open my $fh, '-|', 'make', '-s', '--no-print-directory', '-C', $dir,
        '-f', 'Makefile', '-f', "$work/config.mk", 'jot-template-config' or die $!;
    my @values = <$fh>;
    close $fh or die "Cannot read CPython build configuration: $dir\n";
    chomp @values;
    die "Unexpected build configuration: $dir\n" unless @values == 3;
    my $srcdir = $values[2] =~ m{^/} ? $values[2] : "$dir/$values[2]";
    die "Build must use --source-dir: $dir\n" unless (abs_path($srcdir) // '') eq $source;
    return @values;
}
my ($version, $library) = build_config($build);
my ($native_version) = build_config($native);
my $patchlevel = read_file("$source/Include/patchlevel.h");
my ($major) = $patchlevel =~ /^#define\s+PY_MAJOR_VERSION\s+(\d+)/m;
my ($minor) = $patchlevel =~ /^#define\s+PY_MINOR_VERSION\s+(\d+)/m;
die "CPython 3.11+ sources and matching native/WASI builds are required\n"
    unless defined($major) && defined($minor) && $major == 3 && $minor >= 11
        && $version eq "$major.$minor" && $native_version eq $version;
die "Missing static Python library: $build/$library\n" unless -f "$build/$library";
my $freeze = "$native/Programs/_freeze_module";
die "Build the native Programs/_freeze_module target first\n" unless -x $freeze;

my @modules;
for my $path (sort glob "$source/Lib/*.py") {
    $path =~ m{/([^/]+)\.py$} or die $path;
    push @modules, [$1, $path, 0];
}
for my $package (qw(encodings collections re)) {
    for my $path (sort glob "$source/Lib/$package/*.py") {
        $path =~ m{/([^/]+)\.py$} or die $path;
        push @modules, [$1 eq '__init__' ? $package : "$package.$1", $path, $1 eq '__init__' ? 1 : 0];
    }
}
open my $main, '>', "$work/main.c" or die $!;
print {$main} "#include <Python.h>\n";
my @entries;
print "[python-template] freeze ", scalar(@modules), " modules\n";
for my $index (0 .. $#modules) {
    my ($name, $path, $package) = @{$modules[$index]};
    run($freeze, $name, $path, "$work/module.h");
    my $header = read_file("$work/module.h");
    my ($symbol) = $header =~ /const unsigned char (\w+)\[\]/;
    die "Unsupported _freeze_module output for $name\n" unless defined $symbol;
    # Private symbols avoid collisions with CPython's built-in frozen modules.
    $header =~ s/\b\Q$symbol\E\b/jot_module_$index/g;
    print {$main} $header;
    push @entries, qq({"$name", jot_module_$index, sizeof(jot_module_$index), $package},);
}
my $capacity = 1024 * 1024;
my $begin = "# JOT_PYTHON_SOURCE_V1_BEGIN\nraise RuntimeError(\"unpatched template\")\n";
my $end = 'JOT_PYTHON_SOURCE_V1_END';
print {$main} "static const struct _frozen modules[] = {\n", join("\n", @entries), "\n{0}};\n";
print {$main} "static const char submission[] = {", join(',', unpack('C*', $begin . (' ' x ($capacity - length($begin) - 1)) . "\0$end\0")), "};\n";
print {$main} <<'C';
int main(void) {
    PyImport_FrozenModules = modules;
    PyPreConfig pre;
    PyPreConfig_InitIsolatedConfig(&pre);
    pre.utf8_mode = 1;
    PyStatus status = Py_PreInitialize(&pre);
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    PyConfig config;
    PyConfig_InitIsolatedConfig(&config);
    config.site_import = 0;
    config.write_bytecode = 0;
    config.parse_argv = 0;
    config.module_search_paths_set = 1;
    config.use_hash_seed = 1;
    config.hash_seed = 0;
    status = PyConfig_SetString(&config, &config.program_name, L"jot-python");
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    status = PyConfig_SetBytesString(&config, &config.run_command, submission);
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    status = Py_InitializeFromConfig(&config);
    PyConfig_Clear(&config);
    if (PyStatus_Exception(status)) { Py_ExitStatusException(status); }
    return Py_RunMain();
}
C
close $main or die $!;
my $cc = $args{cc} // "$args{'wasi-sdk'}/bin/clang";
my $sysroot = $args{sysroot} // "$args{'wasi-sdk'}/share/wasi-sysroot";
my @archives;
open my $archives, '-|', 'find', "$build/Modules", '-type', 'f', '-name', '*.a', '-print0' or die $!;
{
    local $/ = "\0";
    while (my $path = <$archives>) { chomp $path; push @archives, $path; }
}
close $archives or die "Cannot find extension archives\n";
@archives = sort @archives;
stage('link-template', $work, {}, $cc, '--target=wasm32-wasip1', "--sysroot=$sysroot", '-O2',
    "-I$source/Include", "-I$build", "$work/main.c", "$build/$library", @archives,
    '-lm', '-ldl', '-lwasi-emulated-getpid', '-lwasi-emulated-signal',
    '-lwasi-emulated-process-clocks', '-Wl,-z,stack-size=1048576',
    '-Wl,--strip-all', '-o', "$work/template.wasm");
sub uleb {
    my ($value) = @_;
    my $bytes = '';
    while ($value >= 128) {
        $bytes .= chr(($value & 127) | 128);
        $value >>= 7;
    }
    return $bytes . chr($value);
}
my $wasm = read_file("$work/template.wasm");
die "Expected a core Wasm module\n" unless substr($wasm, 0, 8) eq "\0asm\x01\0\0\0";
my $offset = index($wasm, $begin);
die "Template must contain exactly one intact source slot\n" if $offset < 0
    || index($wasm, $begin, $offset + 1) >= 0
    || substr($wasm, $offset + $capacity, length($end)) ne $end;
my $name = 'jot.python.source.v1';
my $metadata = uleb(length($name)) . $name . pack('V2', $offset, $capacity);
open my $out, '>:raw', $output or die "$output: $!\n";
print {$out} $wasm, "\0", uleb(length($metadata)), $metadata or die $!;
close $out or die $!;
chmod 0644, $output or die $!;
printf "%s: %d bytes, %d frozen modules (CPython %s)\n", $output, -s $output, scalar(@modules), $version;
print "[python-template] logs: $args{'log-dir'}\n";
