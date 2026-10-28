package io.github.skunkwerkx.hyperuuid.probe;

import io.github.skunkwerkx.hyperuuid.UuidGenerator;
import java.lang.reflect.Method;

/**
 * The module-path child's program. It lives in its own package: the library's package belongs to
 * the automatic module there, so a class of this package name on the class path would be hidden.
 * The backend is read reflectively (an automatic module opens all its packages).
 */
public final class ModuleMain {
    private ModuleMain() {}

    public static void main(String[] args) throws Exception {
        UuidGenerator.newV4();
        Method backend =
                Class.forName("io.github.skunkwerkx.hyperuuid.NativeCore").getDeclaredMethod("backend");
        backend.setAccessible(true);
        System.out.println("backend: " + backend.invoke(null));
        System.out.println("module: " + UuidGenerator.class.getModule().getName());
    }
}
